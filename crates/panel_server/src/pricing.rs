//! A brand's pricing over HTTP (`panel::pricing`): the editor's routes under `/api/v1` behind
//! the conductor's assertion, and the sites' read under `/api/internal`, which has none.
//!
//! ```text
//! GET    /api/internal/brands/{brand}/pricing       the model, or {} — never a 404 or a 5xx
//! ```

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use axum::{
	Extension, Json,
	extract::{Path, State},
	http::Method,
};
use ev_lib::api::{Body, Ids, JsonSchema, Refusal, Refuses};
use jiff::Timestamp;
use panel::{
	Panel,
	place::Expected,
	pricing::{ChangeKind, PricingError, PricingView},
};
use panel_core::pricing::{LocaleTag, ModelPart, PricingModel, parse_locales};
use sa_auth::{Gate, Pricing, Work};
use sa_core::ids::BrandId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
	api::{ApiResult, Caller, Code},
	conductor::{InternalEndpoint, InternalReads},
	http::ApiRoute,
	places::editor,
};

/// How long the sites' read may take before they are answered `{}`: a site gives up at 3 s.
const LIVE_WITHIN: Duration = Duration::from_millis(2500);

pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Gate::USER.needs(Work::Read);
	let edit = Gate::USER.needs(Pricing::Edit);
	vec![
		ApiRoute::new(M::GET, "/pricing", work, list_pricing),
		ApiRoute::new(M::GET, "/pricing/{brand}", work, get_pricing),
		ApiRoute::new(M::GET, "/pricing/{brand}/changes", work, pricing_changes),
		ApiRoute::new(M::POST, "/pricing/{brand}/preview", work, preview_price),
		ApiRoute::new(M::PUT, "/pricing/{brand}", edit, set_pricing),
		ApiRoute::new(M::DELETE, "/pricing/{brand}", edit, remove_pricing),
		ApiRoute::new(M::PUT, "/pricing/{brand}/locales", edit, set_locales),
	]
}

impl Refuses<PricingError> for Code {
	fn refusal(e: PricingError) -> Refusal<Self> {
		match e {
			PricingError::Stale(_) => Refusal::new(Self::Stale, e.to_string()),
			PricingError::Invalid(problems) => match problems.first() {
				Some(first) => Refusal::new(Self::InvalidPricing, first.to_string()),
				None => Self::refusal(eyre::eyre!("a model refused with no problem named")),
			},
			PricingError::Internal(e) => Self::refusal(e),
		}
	}
}

/// A brand's pricing as saved.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Pricing")]
struct ItemDto {
	brand_id: BrandId,
	/// The languages every label must be in; fr and en unless set.
	locales: Vec<String>,
	/// Null when never set or removed: the sites keep their baked one.
	#[schemars(with = "Option<PricingModel>")]
	model: Option<Value>,
	/// Null while never set: what a first save must expect. A removal has a time too.
	updated_at: Option<Timestamp>,
	updated_by: Option<String>,
}

impl From<PricingView> for ItemDto {
	fn from(v: PricingView) -> Self {
		Self {
			brand_id: v.brand,
			locales: v.locales,
			model: v.model,
			updated_at: v.updated_at,
			updated_by: v.updated_by,
		}
	}
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Pricings")]
struct ItemsDto {
	items: Vec<ItemDto>,
}

/// The pricing of every brand the panel knows (a lead's, a place's, a landing's experiments')
/// or has pricing or locales for.
async fn list_pricing(State(panel): State<Panel>) -> ApiResult<Json<ItemsDto>> {
	Ok(Json(ItemsDto {
		items: panel.all_pricing().await?.into_iter().map(ItemDto::from).collect(),
	}))
}

/// `{brand}`.
#[derive(Deserialize, JsonSchema)]
struct BrandPath {
	brand: BrandId,
}

/// A brand's pricing; one never set has its model, `updated_at` and `updated_by` null.
async fn get_pricing(State(panel): State<Panel>, Ids(BrandPath { brand }): Ids<BrandPath>) -> ApiResult<Json<ItemDto>> {
	Ok(Json(panel.pricing(&brand).await?.into()))
}

/// One change of a brand's pricing.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PricingChange")]
struct ChangeDto {
	id: Uuid,
	at: Timestamp,
	by: String,
	kind: ChangeKind,
	/// The saved model's `validFrom`; null for a removal.
	valid_from: Option<String>,
	/// How many needs the saved model prices; null for a removal.
	needs: Option<usize>,
	/// The model the change left; null for a removal.
	#[schemars(with = "Option<PricingModel>")]
	model: Option<Value>,
	/// What it changed of the model before it, in the model's order; null for a first save or
	/// when either side is a removal.
	changed: Option<Vec<ModelPart>>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PricingChanges")]
struct ChangesDto {
	changes: Vec<ChangeDto>,
}

/// A brand's last 50 pricing changes, the latest first; a removal's model, `valid_from` and
/// `needs` null.
async fn pricing_changes(State(panel): State<Panel>, Ids(BrandPath { brand }): Ids<BrandPath>) -> ApiResult<Json<ChangesDto>> {
	let changes = panel
		.pricing_history(&brand)
		.await?
		.into_iter()
		.map(|c| ChangeDto {
			id: c.id,
			at: c.at,
			by: c.by,
			kind: c.kind,
			valid_from: c.valid_from,
			needs: c.needs,
			model: c.model,
			changed: c.changed,
		})
		.collect();
	Ok(Json(ChangesDto { changes }))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SetPricing")]
struct SetBody {
	#[schemars(with = "PricingModel")]
	model: Value,
	/// The `updated_at` the editor read; null (or absent) for pricing never set.
	#[serde(default)]
	expected_updated_at: Option<Timestamp>,
}

/// Saves a brand's price list; its sites take it on their next read.
///
/// 409 `stale` when `expected_updated_at` is not what it stands at; 422 `invalid_pricing` naming
/// the model's first problem by its path.
async fn set_pricing(State(panel): State<Panel>, caller: Caller, Ids(BrandPath { brand }): Ids<BrandPath>, Body(b): Body<SetBody>) -> ApiResult<Json<ItemDto>> {
	let v = panel
		.set_pricing(&editor(&caller), &brand, &b.model, Expected::At(b.expected_updated_at), Timestamp::now())
		.await?;
	Ok(Json(v.into()))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "RemovePricing")]
struct RemoveBody {
	/// Required: taking a brand's prices off its sites over a version the editor did not see is
	/// not something to do by default. Null for pricing never set.
	#[serde(deserialize_with = "Option::deserialize")]
	#[schemars(required)]
	expected_updated_at: Option<Timestamp>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SetLocales")]
struct LocalesBody {
	/// 1 to 8, each once: every label of the brand's model must be in each.
	locales: Vec<LocaleTag>,
}

/// Sets the locales a brand's sites speak. The saved model is not checked again: one missing a
/// new locale's labels is kept for the editor and not served until saved with them.
async fn set_locales(State(panel): State<Panel>, caller: Caller, Ids(BrandPath { brand }): Ids<BrandPath>, Body(b): Body<LocalesBody>) -> ApiResult<Json<ItemDto>> {
	let locales = parse_locales(b.locales.iter().map(LocaleTag::as_str))?;
	Ok(Json(panel.set_brand_locales(&editor(&caller), &brand, &locales, Timestamp::now()).await?.into()))
}

/// Takes a brand's price list off its sites: they keep their baked one.
///
/// 409 as for a PUT.
async fn remove_pricing(State(panel): State<Panel>, caller: Caller, Ids(BrandPath { brand }): Ids<BrandPath>, Body(b): Body<RemoveBody>) -> ApiResult<Json<ItemDto>> {
	let v = panel.remove_pricing(&editor(&caller), &brand, Expected::At(b.expected_updated_at), Timestamp::now()).await?;
	Ok(Json(v.into()))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PricePreviewRequest")]
struct PreviewBody {
	#[schemars(with = "PricingModel")]
	model: Value,
	need: String,
	/// Input id → option id.
	#[serde(default)]
	inputs: BTreeMap<String, String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PricePreview")]
struct PreviewDto {
	/// Integer cents EUR TTC; null when the need is a quote.
	cents: Option<i64>,
}

/// What a model, saved or not, prices a need at with these answers; `cents` null is no price.
///
/// 422 as for a PUT: the draft is held to the brand's locales, as a site would hold it.
async fn preview_price(State(panel): State<Panel>, Ids(BrandPath { brand }): Ids<BrandPath>, Body(b): Body<PreviewBody>) -> ApiResult<Json<PreviewDto>> {
	let cents = panel.preview_price(&brand, &b.model, &b.need, &b.inputs).await?;
	Ok(Json(PreviewDto { cents }))
}

/// What a site gets: the model, or `{}` for none — and `{}` too for a brand id that cannot be
/// one, a store that fails or does not answer in time. The site then keeps its baked model,
/// which is what it would do on an error; a 404 or a 5xx here would only be noise.
pub(crate) async fn live(State(panel): State<Panel>, Extension(reads): Extension<Arc<InternalReads>>, Path(brand): Path<String>) -> Json<Value> {
	let Ok(brand) = BrandId::parse(&brand) else {
		return Json(json!({}));
	};
	reads.record(&brand, InternalEndpoint::Pricing);
	match tokio::time::timeout(LIVE_WITHIN, panel.live_pricing(&brand)).await {
		Ok(Ok(Some(model))) => Json(model),
		Ok(Ok(None)) => Json(json!({})),
		Ok(Err(e)) => {
			crate::report(&e, "serving a brand's pricing to a site");
			Json(json!({}))
		}
		Err(_) => {
			tracing::warn!(%brand, "serving a brand's pricing to a site took too long: answered {{}}");
			Json(json!({}))
		}
	}
}
