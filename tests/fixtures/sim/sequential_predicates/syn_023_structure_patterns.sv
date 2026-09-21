// llg-test-fixture: tests/fixtures/sim/sequential_predicates/syn_023_structure_patterns.sv
// LRM: IEEE 1800-2009 12.6, 12.6.2-12.6.3.
// Recursive fixed structure patterns keep resolved member identity and bind
// only after each source-ordered member check succeeds.
module tb;
    typedef struct packed {
        logic [3:0] state;
        bit signed [3:0] signed_count;
    } packed_t;
    typedef struct packed {
        packed_t inner;
        logic [7:0] tail;
    } nested_t;
    typedef struct {
        logic [7:0] data;
        bit [3:0] tag;
    } unpacked_t;

    packed_t packed_value;
    nested_t nested_value;
    unpacked_t unpacked_value;
    int pass, calls;

    function automatic packed_t sample_packed();
        calls = calls + 1;
        sample_packed = packed_value;
    endfunction

    function automatic unpacked_t sample_unpacked();
        calls = calls + 1;
        sample_unpacked = unpacked_value;
    endfunction

    initial begin
        packed_value = '{state: 4'ha, signed_count: -4'sd2};
        nested_value = '{inner: '{state: 4'ha, signed_count: -4'sd2}, tail: 8'hc3};
        unpacked_value = '{data: 8'h3c, tag: 4'h5};
        pass = 0;

        calls = 0;
        if (sample_packed() matches '{.pos_state, .pos_count}) begin
            if (pos_state !== 4'ha || pos_count !== -4'sd2)
                $fatal(1, "positional structure binding");
            pass++;
        end
        if (calls != 1)
            $fatal(1, "positional source evaluated more than once");

        if (packed_value matches '{signed_count: .named_count, state: 4'ha}) begin
            if (named_count !== -4'sd2)
                $fatal(1, "named reordered structure binding");
            pass++;
        end

        if (nested_value matches
            '{tail: 8'hc3, inner: '{signed_count: .nested_count}}) begin
            if (nested_count !== -4'sd2)
                $fatal(1, "nested structure binding");
            pass++;
        end

        if (nested_value matches
            '{inner: '{signed_count: .filtered}, tail: .tail_bound} &&&
            filtered == -4'sd2 && tail_bound == 8'hc3) begin
            pass++;
        end

        calls = 0;
        if (sample_unpacked() matches '{tag: 4'h5}) begin
            pass++;
        end
        if (calls != 1)
            $fatal(1, "unpacked source evaluated more than once");

        if (nested_value matches '{inner: .*, tail: 8'hc3})
            pass++;

        if (pass != 6)
            $fatal(1, "structure pattern count %0d", pass);
        $display("structure_patterns=pass checks=%0d calls=%0d", pass, calls);
        $finish(0);
    end
endmodule
