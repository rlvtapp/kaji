  class ResponseDecodeError < TypeError
    attr_reader :path, :expected
    def initialize(path, expected)
      @path, @expected = path, expected
      super("Kaji response decoding failed at #{path}: expected #{expected}")
    end
  end

  module ResponseValidation
    module_function
    def response_shape(schemas, status, content_type)
      return nil unless schemas && status.between?(200, 299) && status != 204
      media = content_type.to_s.split(';', 2).first.to_s.strip.downcase
      return nil unless media == 'application/json' || media.end_with?('+json')
      candidates = schemas[status.to_s] || schemas["#{status / 100}XX"] || schemas['default'] || schemas['DEFAULT'] || {}
      candidates[media] || candidates["#{media.split('/').first}/*"] || candidates['*/*']
    end

    def decode(raw, schema, refs)
      raise ResponseDecodeError.new('$', 'response at most 10 MiB') unless raw.is_a?(String) && raw.bytesize <= 10 * 1024 * 1024
      begin
        value = JSON.parse(raw, max_nesting: 128, allow_nan: false)
      rescue JSON::ParserError, EncodingError, ArgumentError
        raise ResponseDecodeError.new('$', 'one valid bounded JSON value')
      end
      assert_shape(value, schema, refs)
      value
    end

    def assert_shape(value, schema, refs, path = '$', depth = 0)
      return unless schema
      raise ResponseDecodeError.new(path, 'bounded response structure') if depth > 128
      kind = schema['kind'] || 'any'
      return if value.nil? && (schema['nullable'] || kind == 'null' || kind == 'any')
      if schema['ref']
        target = refs[schema['ref']]
        raise ResponseDecodeError.new(path, 'resolved response schema') unless target
        return assert_shape(value, target, refs, path, depth + 1)
      end
      if schema['variants']
        if kind == 'allOf'
          schema['variants'].each { |variant| assert_shape(value, variant, refs, path, depth + 1) }
          return
        end
        schema['variants'].each do |variant|
          begin
            assert_shape(value, variant, refs, path, depth + 1)
            return
          rescue ResponseDecodeError
          end
        end
        raise ResponseDecodeError.new(path, 'declared union shape')
      end
      accepted = case kind
      when 'any' then true
      when 'null' then value.nil?
      when 'string' then value.is_a?(String)
      when 'boolean' then value == true || value == false
      when 'integer' then value.is_a?(Integer)
      when 'number' then value.is_a?(Integer) || (value.is_a?(Float) && value.finite?)
      when 'object' then value.is_a?(Hash)
      when 'array' then value.is_a?(Array)
      else false
      end
      raise ResponseDecodeError.new(path, kind) unless accepted
      if kind == 'object'
        fields = schema['fields'] || {}
        (schema['required'] || []).each do |key|
          raise ResponseDecodeError.new("#{path}[#{JSON.generate(key)}]", 'required property') unless value.key?(key)
        end
        value.each do |key, item|
          # Dynamic extra property names can themselves contain secrets. Keep
          # diagnostics bounded and avoid echoing undeclared response keys.
          child = fields.key?(key) ? "#{path}[#{JSON.generate(key)}]" : "#{path}[additional]"
          assert_shape(item, fields[key] || schema['additional'], refs, child, depth + 1)
        end
      elsif kind == 'array'
        value.each_with_index { |item, index| assert_shape(item, schema['items'], refs, "#{path}[#{index}]", depth + 1) }
      end
    end
  end
