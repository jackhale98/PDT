# PDT Quote Entity (Supplier Quotations)

This document describes the Quote entity type in PDT (Plain-text Product Development Toolkit).

## Overview

Quotes represent supplier quotations for components or assemblies. They track pricing information including quantity-based price breaks, lead times, NRE (non-recurring engineering) costs, and link to Supplier entities for contact details. Quotes enable comparison shopping and procurement planning.

**Note**: Quotes reference Supplier entities by ID. Create suppliers first using `pdt sup new`, then link quotes to them.

## Entity Type

- **Prefix**: `QUOT`
- **File extension**: `.pdt.yaml`
- **Directory**: `bom/quotes/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (QUOT-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `supplier` | string | Supplier ID (SUP-... or SUP@N) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Item Reference (One Required)

Quotes must reference either a component OR an assembly (not both):

| Field | Type | Description |
|-------|------|-------------|
| `component` | string | Component ID this quote is for (mutually exclusive with assembly) |
| `assembly` | string | Assembly ID this quote is for (mutually exclusive with component) |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `quote_ref` | string | Supplier's quote reference number |
| `description` | string | Detailed notes or terms |
| `currency` | enum | Currency code: `USD`, `EUR`, `GBP`, `CNY`, `JPY` |
| `price_breaks` | array[PriceBreak] | Quantity-based pricing tiers |
| `moq` | integer | Minimum order quantity |
| `tooling_cost` | number | One-time tooling cost |
| `nre_costs` | array[NreCost] | Non-recurring engineering costs |
| `lead_time_days` | integer | Standard lead time in days |
| `quote_date` | date | Date quote was received |
| `valid_until` | date | Quote expiration date |
| `quote_status` | enum | `pending`, `received`, `accepted`, `rejected`, `expired` |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### PriceBreak Object

| Field | Type | Description |
|-------|------|-------------|
| `min_qty` | integer | Minimum quantity for this price tier |
| `unit_price` | number | Unit price at this quantity |
| `lead_time_days` | integer | Lead time at this quantity (optional) |

### NreCost Object

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Description of the NRE item |
| `cost` | number | Cost amount |
| `one_time` | boolean | Is this a one-time cost? |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.related_quotes` | array[EntityId] | Related/competing quotes |

## Example

### Quote for a Component

```yaml
# Quote: Acme Corp Quote for Widget Bracket
# Created by PDT - Plain-text Product Development Toolkit

id: QUOT-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: "Acme Corp Quote"

# Link to supplier entity
supplier: SUP-01HC2JB7SMQX7RS1Y0GFKBHPTA

# Component this quote is for
component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTC

# Supplier's quote reference
quote_ref: "ACM-Q-2024-001"

description: |
  Quote for Widget Bracket.
  Includes quantity discounts.
  Terms: Net 30.

currency: USD

price_breaks:
  - min_qty: 1
    unit_price: 15.00
    lead_time_days: 14
  - min_qty: 100
    unit_price: 12.50
    lead_time_days: 14
  - min_qty: 500
    unit_price: 10.00
    lead_time_days: 21
  - min_qty: 1000
    unit_price: 8.50
    lead_time_days: 28

moq: 1
tooling_cost: 500.00
lead_time_days: 14

quote_date: 2024-01-15
valid_until: 2024-04-15

quote_status: received
tags: [bracket, aluminum]
status: draft

links:
  related_quotes: []

created: 2024-01-15T10:30:00Z
author: Jack Hale
entity_revision: 1
```

### Quote for an Assembly

```yaml
# Quote: Contract Manufacturing Quote for Main Assembly
# Created by PDT - Plain-text Product Development Toolkit

id: QUOT-01HC2JB7SMQX7RS1Y0GFKBHPTE
title: "Contract Mfg Quote - Main Assembly"

# Link to supplier entity
supplier: SUP-01HC2JB7SMQX7RS1Y0GFKBHPTB

# Assembly this quote is for
assembly: ASM-01HC2JB7SMQX7RS1Y0GFKBHPTF

quote_ref: "BM-2024-0042"

description: |
  Complete assembly and test.
  Includes all labor and inspection.

currency: USD

price_breaks:
  - min_qty: 1
    unit_price: 250.00
    lead_time_days: 21
  - min_qty: 50
    unit_price: 200.00
    lead_time_days: 28

tooling_cost: 2500.00

nre_costs:
  - description: "Fixture development"
    cost: 1500.00
    one_time: true
  - description: "Test programming"
    cost: 800.00
    one_time: true

lead_time_days: 21
quote_status: pending
status: draft

created: 2024-01-20T14:00:00Z
author: Jack Hale
entity_revision: 1
```

## CLI Commands

### Create a new quote

```bash
# First create a supplier
pdt sup new --name "Acme Corp" --no-edit

# Create quote for a component
pdt quote new --component CMP@1 --supplier SUP@1 --title "Bracket Quote"

# Create quote for an assembly
pdt quote new --assembly ASM@1 --supplier SUP@1 --title "Assembly Quote"

# With price and lead time
pdt quote new -c CMP@1 -s SUP@1 --price 12.50 --lead-time 14

# Interactive mode (prompts for all fields)
pdt quote new -i

# Create and open in editor
pdt quote new -c CMP@1 -s SUP@1 --edit

# Create without opening editor
pdt quote new -c CMP@1 -s SUP@1 --no-edit
```

### List quotes

```bash
# List all quotes
pdt quote list

# Filter by quote status (use -Q for quote status)
pdt quote list -Q pending
pdt quote list -Q received
pdt quote list -Q accepted

# Filter by entity status
pdt quote list -s draft
pdt quote list -s approved

# Filter by component
pdt quote list --component CMP@1
pdt quote list -c CMP@1

# Filter by assembly
pdt quote list --assembly ASM@1
pdt quote list -a ASM@1

# Filter by supplier
pdt quote list --supplier SUP@1
pdt quote list -S SUP@1

# Search in title
pdt quote list --search "bracket"

# Sort and limit
pdt quote list --sort supplier
pdt quote list --sort title
pdt quote list --limit 10
pdt quote list --reverse

# Count only
pdt quote list --count

# Output formats
pdt quote list -o json
pdt quote list -o csv
pdt quote list -o md
pdt quote list -o yaml
```

### Show quote details

```bash
# Show by ID (partial match supported)
pdt quote show QUOT-01HC2

# Show using short ID
pdt quote show QUOT@1

# Output as JSON
pdt quote show QUOT@1 -o json

# Output as YAML
pdt quote show QUOT@1 -o yaml
```

### Edit a quote

```bash
# Open in editor
pdt quote edit QUOT-01HC2

# Using short ID
pdt quote edit QUOT@1
```

### Compare quotes

Compare all quotes for a specific component or assembly:

```bash
# Compare quotes for a component
pdt quote compare CMP@1

# Compare quotes for an assembly
pdt quote compare ASM@1

# Output as JSON
pdt quote compare CMP@1 -o json

# Output as YAML
pdt quote compare CMP@1 -o yaml
```

The compare command sorts quotes by unit price (lowest first) and shows a summary highlighting the best price.

## Quote Status Values

| Status | Description |
|--------|-------------|
| **pending** | Quote requested but not yet received |
| **received** | Quote received, under review |
| **accepted** | Quote accepted for use |
| **rejected** | Quote rejected (price, lead time, etc.) |
| **expired** | Quote has passed valid_until date |

## Currency Support

| Code | Currency |
|------|----------|
| `USD` | US Dollar (default) |
| `EUR` | Euro |
| `GBP` | British Pound |
| `CNY` | Chinese Yuan |
| `JPY` | Japanese Yen |

## Best Practices

### Quote Management

1. **Create suppliers first** - Use `pdt sup new` before creating quotes
2. **Get multiple quotes** - Always get at least 2-3 quotes for comparison
3. **Track expiration** - Set `valid_until` and review before expiry
4. **Use price breaks** - Document all quantity tiers offered
5. **Include NRE** - Track all non-recurring costs separately
6. **Reference supplier quote** - Store `quote_ref` for traceability

### Pricing Comparison

1. **Consider total cost** - Include tooling and NRE in comparisons
2. **Match quantities** - Compare at relevant production volumes
3. **Factor lead times** - Longer lead times may affect schedule
4. **Document terms** - Note payment terms in description

### Workflow

1. Create supplier if not exists (`pdt sup new`)
2. Create quotes when RFQs are sent (`pending`)
3. Update to `received` when quote arrives
4. Review and compare quotes (`pdt quote compare`)
5. Mark winning quote as `accepted`
6. Mark others as `rejected` or let expire

## Related Entities

- **Supplier (SUP)**: Referenced by quotes for contact and company information
- **Component (CMP)**: Items that quotes can be for
- **Assembly (ASM)**: Items that quotes can be for

## Validation

Quotes are validated against a JSON Schema:

```bash
# Validate all project files
pdt validate

# Validate specific file
pdt validate bom/quotes/QUOT-01HC2JB7SMQX7RS1Y0GFKBHPTD.pdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `QUOT-[A-Z0-9]{26}` pattern
2. **Title**: Required, 1-200 characters
3. **Item Reference**: Either `component` or `assembly` should be set (not both)
4. **Supplier**: Must be a valid supplier ID
5. **Quote Status**: If specified, must be valid enum value
6. **Currency**: If specified, must be valid enum value
7. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`

## JSON Schema

The full JSON Schema for quotes is available at:

```
pdt/schemas/quot.schema.json
```
