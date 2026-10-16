//! What a run on the OpenRouter road spent, as the tap read it going by (docs/cache-fallback.md, 「The
//! OpenRouter substitute is the dsh road」): dsh prints only the answer, so the figures the gateway reported
//! are laid on its reply here. A job's own turn on the road is the job unit's.

use super::openrouter_tap::Observation;
use crate::door::record::{ModelSpend, Reply, RequestUsage};

/// dsh's answer with the figures the tap read beside it, filed under the road the reply came on; one whose
/// stream carried no usage keeps what dsh gave, so nothing is recorded that was not reported.
pub fn observed(mut reply: Reply, model: &str, seen: &Observation) -> Reply {
  let Some(total) = seen.total else { return reply };

  reply.spend = vec![ModelSpend {
    mode: reply.mode.clone(),
    model: model.to_owned(),
    tokens: total.input_tokens + total.output_tokens + total.cache_read_tokens + total.cache_write_tokens,
    input_tokens: total.input_tokens,
    output_tokens: total.output_tokens,
    cache_read_tokens: total.cache_read_tokens,
    cache_write_tokens: total.cache_write_tokens,
    cost: total.cost,
  }];
  reply.last_request_usage = seen.last.map(|last| RequestUsage {
    input_tokens: last.input_tokens,
    cache_read_tokens: last.cache_read_tokens,
    cache_write_tokens: last.cache_write_tokens,
    output_tokens: last.output_tokens,
  });
  reply
}
