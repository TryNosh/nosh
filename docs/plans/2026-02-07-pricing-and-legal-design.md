# Pricing Strategy and Legal Framework

## Overview

This document defines nosh's pricing structure, token mechanics, licensing, and legal policies.

## Pricing Tiers

| Tier | Price | Pool Tokens | Simple Queries | Agentic Queries |
|------|-------|-------------|----------------|-----------------|
| Free | $0/mo | 5k | ~125 | — |
| Lite | $2.99/mo | 250k | ~6,250 | ~50 |
| Pro | $9.99/mo | 1M | ~25,000 | ~200 |
| Power | $19.99/mo | 3M | ~75,000 | ~600 |

### Token Pack

- **Price:** $2.99
- **Tokens:** 125k pool tokens
- **Eligibility:** Paid subscribers only (Lite, Pro, Power)
- **Expiry:** Never expires, stacks with subscription

## Token Mechanics

### Dual-Model Architecture

| Mode | Model | Env Variable | Cost/M Input | Cost/M Output |
|------|-------|--------------|--------------|---------------|
| Simple (`?`) | Small | `MISTRAL_MODEL_SMALL` | $0.10 | $0.30 |
| Agentic (`??`) | Large | `MISTRAL_MODEL_LARGE` | $0.40 | $2.00 |

Each endpoint uses a different model to optimize cost vs capability. Falls back to `MISTRAL_MODEL` if specific vars not set.

### Pool Token Conversion

To normalize costs across modes, pool tokens are deducted at different rates:

| Mode | Actual Tokens | Pool Tokens Deducted | Ratio |
|------|---------------|---------------------|-------|
| Simple | 200 | 40 | 5:1 |
| Agentic | 5,000 | 5,000 | 1:1 |

Simple mode is 5x more efficient in pool token usage, reflecting its lower infrastructure cost.

### Example Usage

A user with 250k pool tokens (Lite tier) can use:
- **All simple:** 6,250 queries (250k / 40 tokens per query)
- **All agentic:** 50 queries (250k / 5k tokens per query)
- **Mixed:** Any combination within the pool

## Free Tier Constraints

| Constraint | Value |
|------------|-------|
| Pool tokens per user | 5k/month |
| Agentic mode | Disabled |
| Global pool cap | 40M tokens |
| Max free users | ~8,000 |
| Monthly cost cap | ~$40 |
| Token pack eligible | No |

### Fraud Prevention

Free accounts require:
- Email verification
- IP tracking
- Machine code fingerprinting

This prevents abuse while maintaining a generous free tier for legitimate users.

## Unit Economics

### Per-Tier Margins

| Tier | Price | Stripe Fee | Max AI Cost | Min Profit | Margin |
|------|-------|------------|-------------|------------|--------|
| Lite | $2.99 | $0.39 | $0.30 | $2.30 | 77% |
| Pro | $9.99 | $0.59 | $1.20 | $8.20 | 82% |
| Power | $19.99 | $0.88 | $3.60 | $15.51 | 78% |
| Token pack | $2.99 | $0.39 | $0.15 | $2.45 | 82% |

### Cost Assumptions

- Stripe: 2.9% + $0.30 per transaction
- AI costs calculated at worst-case (100% agentic usage)
- Blended small model cost: ~$0.20/M tokens
- Blended large model cost: ~$1.20/M tokens

## Licensing

### Type

**Proprietary / Closed Source**

nosh is proprietary software. The source code is not publicly available.

### Rationale

- Prevents commercial competitors from forking or wrapping nosh
- Maintains control over product direction
- Aligns with industry standard (e.g., Warp)

## Ollama Support

**Decision: Removed**

Local AI via Ollama is not supported.

### Rationale

1. **Competition risk** — third parties could wrap nosh with "free AI" marketing
2. **Support burden** — users blame nosh for slow/poor local model performance
3. **User experience** — local models require RAM, are slower, and produce worse results
4. **Simplicity** — one AI backend (cloud) simplifies product and support

## Terms of Use

### Permitted

- Account sharing between individuals
- Personal and commercial use within subscription limits
- Integration into personal workflows

### Prohibited

1. **Abuse** — using AI for spam, malware, harassment, or illegal activity
2. **Reverse engineering** — decompiling, disassembling, or extracting source code
3. **Reselling** — reselling nosh access or wrapping it in another product
4. **Automated abuse** — bot farms, excessive automated requests, rate limit circumvention

### Enforcement

Violations may result in account suspension or termination without refund.

## Privacy Policy

### Principles

1. **Generic provider language** — refer to "third-party AI service providers" (not specific vendors)
2. **Future-proof** — broad enough to cover additional data collection
3. **Transparent** — disclose categories of data, not exhaustive implementation details

### Data Sent to AI Providers

| Mode | Data Transmitted |
|------|------------------|
| Simple (`?`) | Query text, shell context (directory, recent commands) |
| Agentic (`??`) | Query text, shell context, command outputs |

### Data Collection by nosh

| Category | Current | Future |
|----------|---------|--------|
| Account info | Email, payment (via Stripe) | Same |
| Usage metrics | Token consumption | May expand |
| Shell data | Not collected | May collect for product improvement |

### User Rights

- Data deletion upon account termination
- Opt-out of future analytics (when implemented)

## Implementation Checklist

### Pricing System

- [ ] Update subscription tiers in Stripe
- [ ] Implement 5:1 token ratio for simple mode
- [ ] Add token pack purchase (subscriber-only check)
- [ ] Implement global free tier pool cap (40M tokens)
- [ ] Add fraud prevention for free signups

### Legal Pages

- [ ] Write Terms of Use
- [ ] Write Privacy Policy
- [ ] Add legal page links to website footer
- [ ] Require acceptance on signup

### Product Changes

- [ ] Remove Ollama integration from codebase
- [ ] Remove Ollama from documentation
- [ ] Update `--setup` flow to remove Ollama option
- [ ] Update pricing.mdx with new tiers

### Documentation

- [ ] Update pricing page
- [ ] Update FAQ
- [ ] Add token ratio explanation to docs
