# PDT Supplier Entity (Approved Suppliers)

This document describes the Supplier entity type in PDT (Plain-text Product Development Toolkit).

## Overview

Suppliers represent approved vendors for components and assemblies. They store contact information, addresses, quality certifications, and manufacturing capabilities. Suppliers are referenced by Quote entities to link quotations to vendor details.

## Entity Type

- **Prefix**: `SUP`
- **File extension**: `.pdt.yaml`
- **Directory**: `bom/suppliers/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (SUP-[26-char ULID]) |
| `name` | string | Company name |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `short_name` | string | Short display name (e.g., "Acme" instead of "Acme Corporation Inc.") |
| `website` | string | Company website URL |
| `contacts` | array[Contact] | Contact people at the supplier |
| `addresses` | array[Address] | Physical addresses |
| `payment_terms` | string | Default payment terms (e.g., "Net 30") |
| `currency` | enum | Preferred currency: `USD`, `EUR`, `GBP`, `CNY`, `JPY` |
| `certifications` | array[Certification] | Quality certifications |
| `capabilities` | array[Capability] | Manufacturing capabilities |
| `notes` | string | Additional notes about the supplier |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Contact Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Contact person name |
| `role` | string | Role/title (e.g., "Sales", "Engineering", "Quality") |
| `email` | string | Email address |
| `phone` | string | Phone number |
| `primary` | boolean | Is this the primary contact? |

### Address Object

| Field | Type | Description |
|-------|------|-------------|
| `type` | enum | `headquarters`, `manufacturing`, `shipping`, `billing` |
| `street` | string | Street address |
| `city` | string | City |
| `state` | string | State/Province |
| `postal` | string | Postal/ZIP code |
| `country` | string | Country |

### Certification Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Certification name (e.g., "ISO 9001:2015", "AS9100D") |
| `expiry` | date | Expiration date |
| `certificate_number` | string | Certificate number |

### Capability Values

| Value | Description |
|-------|-------------|
| `machining` | CNC machining |
| `sheet_metal` | Sheet metal fabrication |
| `casting` | Metal casting |
| `injection` | Injection molding |
| `extrusion` | Extrusion |
| `pcb` | PCB fabrication |
| `pcb_assembly` | PCB assembly (PCBA) |
| `cable_assembly` | Cable/harness assembly |
| `assembly` | Mechanical assembly |
| `testing` | Test services |
| `finishing` | Surface finishing (plating, painting, etc.) |
| `packaging` | Packaging services |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.approved_for` | array[EntityId] | Components this supplier is approved for |

## Example

```yaml
# Supplier: Acme Manufacturing Corp
# Created by PDT - Plain-text Product Development Toolkit

id: SUP-01HC2JB7SMQX7RS1Y0GFKBHPTA
name: "Acme Manufacturing Corp"
short_name: "Acme"
website: "https://acme-mfg.com"

contacts:
  - name: "John Smith"
    role: "Sales Manager"
    email: "john.smith@acme-mfg.com"
    phone: "+1-555-123-4567"
    primary: true
  - name: "Jane Doe"
    role: "Quality Engineer"
    email: "jane.doe@acme-mfg.com"
    phone: "+1-555-123-4568"

addresses:
  - type: headquarters
    street: "123 Industrial Way"
    city: "San Francisco"
    state: "CA"
    postal: "94102"
    country: "USA"
  - type: manufacturing
    street: "456 Factory Lane"
    city: "Oakland"
    state: "CA"
    postal: "94601"
    country: "USA"

payment_terms: "Net 30"
currency: USD

certifications:
  - name: "ISO 9001:2015"
    expiry: 2026-06-30
    certificate_number: "QMS-2024-12345"
  - name: "AS9100D"
    expiry: 2025-12-31
    certificate_number: "AS-2023-98765"

capabilities:
  - machining
  - sheet_metal
  - assembly
  - finishing

notes: |
  Preferred supplier for precision machined parts.
  Good quality and competitive pricing.
  Lead times typically 2-3 weeks.

tags: [preferred, machining, local]
status: approved

links:
  approved_for:
    - CMP-01HC2JB7SMQX7RS1Y0GFKBHPTC
    - CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD

created: 2024-01-10T09:00:00Z
author: Jack Hale
entity_revision: 1
```

## CLI Commands

### Create a new supplier

```bash
# Create with name only
pdt sup new --name "Acme Manufacturing Corp"

# Create with additional details
pdt sup new --name "Acme Manufacturing Corp" --short-name "Acme" --website "https://acme.com"

# Create with payment terms
pdt sup new -n "Acme Corp" --payment-terms "Net 30"

# Interactive mode (prompts for fields)
pdt sup new -i

# Create and open in editor
pdt sup new -n "Acme Corp" --edit

# Create without opening editor
pdt sup new -n "Acme Corp" --no-edit
```

### List suppliers

```bash
# List all suppliers
pdt sup list

# Filter by status
pdt sup list -s draft
pdt sup list -s approved

# Filter by capability
pdt sup list -c machining
pdt sup list -c pcb_assembly
pdt sup list --capability injection

# Search in name and notes
pdt sup list --search "acme"

# Sort and limit
pdt sup list --sort name
pdt sup list --sort created
pdt sup list --limit 10
pdt sup list --reverse

# Count only
pdt sup list --count

# Output formats
pdt sup list -o json
pdt sup list -o csv
pdt sup list -o md
pdt sup list -o yaml
```

### Show supplier details

```bash
# Show by ID (partial match supported)
pdt sup show SUP-01HC2

# Show using short ID
pdt sup show SUP@1

# Output as JSON
pdt sup show SUP@1 -o json

# Output as YAML
pdt sup show SUP@1 -o yaml
```

### Edit a supplier

```bash
# Open in editor
pdt sup edit SUP-01HC2

# Using short ID
pdt sup edit SUP@1
```

## Currency Support

| Code | Currency |
|------|----------|
| `USD` | US Dollar (default) |
| `EUR` | Euro |
| `GBP` | British Pound |
| `CNY` | Chinese Yuan |
| `JPY` | Japanese Yen |

## Best Practices

### Supplier Management

1. **Complete profiles** - Fill in contacts, addresses, and certifications
2. **Track certifications** - Monitor expiration dates for quality certs
3. **Document capabilities** - List all manufacturing capabilities
4. **Use short names** - Makes lists and displays more readable
5. **Add notes** - Document preferences, issues, and observations

### Workflow

1. Create supplier when first contacting vendor
2. Add contacts as relationships develop
3. Update certifications when received
4. Link approved components via `links.approved_for`
5. Create quotes referencing the supplier ID

### Integration with Quotes

Suppliers are referenced by Quote entities:

```bash
# Create supplier
pdt sup new -n "Acme Corp" --no-edit
# Output: Created supplier SUP@1

# Create quote referencing supplier
pdt quote new -c CMP@1 -s SUP@1 --title "Bracket Quote"
```

## Related Entities

- **Quote (QUOT)**: References suppliers for quotation details
- **Component (CMP)**: Can be linked via `approved_for`

## Validation

Suppliers are validated against a JSON Schema:

```bash
# Validate all project files
pdt validate

# Validate specific file
pdt validate bom/suppliers/SUP-01HC2JB7SMQX7RS1Y0GFKBHPTA.pdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `SUP-[A-Z0-9]{26}` pattern
2. **Name**: Required
3. **Capability**: If specified, must be valid enum values
4. **Currency**: If specified, must be valid enum value
5. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`

## JSON Schema

The full JSON Schema for suppliers is available at:

```
pdt/schemas/sup.schema.json
```
