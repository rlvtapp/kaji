use super::*;

pub(crate) fn render_models(api: &Api, module: &str) -> String {
    let mut out = format!("{NOTICE}require \"json\"\n\nmodule {module}\n  module Models\n");
    out.push_str(r#"    # Lazy aliases avoid evaluating forward constants or Ruby's unsupported class union operator.
    def self.wire_alias(shape)
      Class.new do
        define_singleton_method(:wire_shape) { shape }
        define_singleton_method(:from_hash) { |value| Models.decode_model_value(value, shape) }
        if shape && shape[0] == 'ref'
          define_singleton_method(:new) do |*args, **kwargs|
            target = Models.const_get(shape[1], false)
            raise ArgumentError, 'cyclic model alias' if target.equal?(self)
            target.new(*args, **kwargs)
          end
        end
      end
    end
    def self.decode_model_value(value, shape)
      return value if value.nil? || shape.nil?
      kind, inner = shape
      if kind == 'ref'
        model = const_get(inner, false) if const_defined?(inner, false)
        return model.from_hash(value) if model && model.respond_to?(:from_hash) && value.is_a?(Hash)
      elsif kind == 'array' && value.is_a?(Array)
        return value.map { |item| decode_model_value(item, inner) }
      elsif kind == 'object' && value.is_a?(Hash)
        return value if inner.empty?
        return value.each_with_object({}) { |(name, item), output| output[name] = decode_model_value(item, inner[name]) }
      end
      value
    end
    def self.to_wire(value)
      return value.map { |item| to_wire(item) } if value.is_a?(Array)
      return value.each_with_object({}) { |(name, item), output| output[name] = to_wire(item) } if value.is_a?(Hash)
      return value.to_h if value.respond_to?(:to_h) && !value.nil?
      value
    end

"#);
    for schema in &api.schemas {
        out.push_str(&render_model(api, schema));
    }
    out.push_str("  end\nend\n");
    out
}

pub(crate) fn ruby_decode_shape(value: &SchemaValue) -> String {
    match &value.kind {
        SchemaKind::Reference { reference } => format!(
            "[\"ref\", {}]",
            ruby_string(&pascal_case(
                reference.rsplit('/').next().unwrap_or(reference)
            ))
        ),
        SchemaKind::Array { items } => format!("[\"array\", {}]", ruby_decode_shape(items)),
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => format!(
            "[\"union\", [{}]]",
            variants
                .iter()
                .map(ruby_decode_shape)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SchemaKind::Boolean => "[\"boolean\", nil]".into(),
        SchemaKind::Object { fields, .. } => format!(
            "[\"object\", {{{}}}]",
            fields
                .iter()
                .map(|field| format!(
                    "{} => {}",
                    ruby_string(&field.name),
                    ruby_decode_shape(&field.value)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => "nil".into(),
    }
}

pub(crate) fn render_model(api: &Api, schema: &Schema) -> String {
    let name = pascal_case(&schema.name);
    let mut out = String::new();
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let mut extra_name = "additional_properties".to_owned();
            while fields
                .iter()
                .any(|field| ruby_field_identifier(fields, field) == extra_name)
            {
                extra_name.insert(0, '_');
            }
            let mut present_name = "__poolster_present_wire_fields".to_owned();
            while fields
                .iter()
                .any(|field| ruby_field_identifier(fields, field) == present_name)
                || present_name == extra_name
            {
                present_name.insert(0, '_');
            }
            out = format!("    class {name}\n");
            if fields.is_empty() {
                let _ = writeln!(out, "      attr_reader :{extra_name}");
            } else {
                let attrs = fields
                    .iter()
                    .map(|field| format!(":{}", ruby_field_identifier(fields, field)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(out, "      attr_reader {attrs}");
                let _ = writeln!(out, "      attr_reader :{extra_name}");
            }
            let args = fields
                .iter()
                .map(|field| format!("{}: nil", ruby_field_identifier(fields, field)))
                .collect::<Vec<_>>()
                .join(", ");
            let extra = if args.is_empty() {
                &format!("{extra_name}: {{}}")
            } else {
                &format!(", {extra_name}: {{}}")
            };
            let _ = writeln!(out, "      def initialize({args}{extra})");
            for field in fields {
                let id = ruby_field_identifier(fields, field);
                let _ = writeln!(out, "        @{id} = {id}");
            }
            let _ = writeln!(out, "        @{extra_name} = {extra_name} || {{}}");
            out.push_str("      end\n\n      def self.from_hash(value)\n        return value unless value.is_a?(Hash)\n");
            let values = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: Models.decode_model_value(value[{}], {})",
                        ruby_field_identifier(fields, field),
                        ruby_string(&field.name),
                        ruby_decode_shape(&field.value)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            {
                let known = fields
                    .iter()
                    .map(|field| ruby_string(&field.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                let values = if values.is_empty() {
                    String::new()
                } else {
                    format!("{values}, ")
                };
                let extra_decode = match additional_properties {
                    AdditionalProperties::Schema { value } => format!(
                        ".transform_values {{ |item| Models.decode_model_value(item, {}) }}",
                        ruby_decode_shape(value)
                    ),
                    _ => String::new(),
                };
                let _ = writeln!(
                    out,
                    "        instance = new({values}{extra_name}: value.reject {{ |key, _| [{known}].include?(key) }}{extra_decode})"
                );
            }
            let _ = writeln!(
                out,
                "        instance.instance_variable_set(:@{present_name}, value.keys)\n        instance"
            );
            let known = fields
                .iter()
                .map(|field| ruby_string(&field.name))
                .collect::<Vec<_>>()
                .join(", ");
            let mut helper = "with_present_fields".to_owned();
            while fields
                .iter()
                .any(|field| ruby_field_identifier(fields, field) == helper)
            {
                helper.push('_');
            }
            let _ = writeln!(
                out,
                "      end\n\n      # Return a copy with explicit nulls present at these wire keys.\n      def {helper}(*names)\n        keys = names.map(&:to_s)\n        raise ArgumentError, 'unknown model wire field' unless (keys - [{known}]).empty?\n        copy = dup\n        copy.instance_variable_set(:@{present_name}, ((@{present_name} || []) + keys).uniq)\n        copy\n      end\n\n      def to_h\n        value = {{}}"
            );
            {
                let known = fields
                    .iter()
                    .map(|field| ruby_string(&field.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "        value.merge!({extra_name}.reject {{ |key, _| [{known}].include?(key) }})"
                );
            }
            for field in fields {
                let id = ruby_field_identifier(fields, field);
                let condition = if field.required
                    && !response_validation::write_only(api, &field.value, &mut Default::default())
                {
                    String::new()
                } else {
                    format!(
                        " unless @{id}.nil? && !(@{present_name} || []).include?({})",
                        ruby_string(&field.name)
                    )
                };
                let _ = writeln!(
                    out,
                    "        value[{}] = Models.to_wire(@{id}){condition}",
                    ruby_string(&field.name)
                );
            }
            out.push_str("        value\n      end\n    end\n\n");
        }
        SchemaKind::Reference { .. }
        | SchemaKind::OneOf { .. }
        | SchemaKind::AnyOf { .. }
        | SchemaKind::Boolean => {
            let _ = writeln!(
                out,
                "    {name} = Models.wire_alias({})\n",
                ruby_decode_shape(&schema.value)
            );
        }
        _ => {
            let ruby_type = ruby_type(&schema.value);
            let _ = writeln!(out, "    {name} = {ruby_type}\n");
        }
    }
    out
}
