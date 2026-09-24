// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/typed_scope_remainders.sv
package typed_scope_remainder_types;
    typedef enum logic [3:0] { PHASE_IDLE = 4'h1, PHASE_RUN = 4'ha } phase_t;
    typedef struct packed { logic [3:0] tag; logic [3:0] data; } packed_t;
    typedef logic [3:0] nibbles_t [0:1];
    typedef union packed { logic [7:0] bits; logic [1:0][3:0] halves; } union_t;
    typedef struct { logic [3:0] tag; logic [3:0] data; } record_t;
endpackage

interface typed_scope_sources_if;
    import typed_scope_remainder_types::*;

    // SYN038-GAP-TY-enum__HC-interface: complete enum source and destination.
    phase_t enum_source = PHASE_RUN;
    phase_t enum_copy;

    // SYN038-GAP-TY-packed_struct__HC-interface: complete packed struct value.
    packed_t packed_source = '{tag: 4'h2, data: 4'hb};
    packed_t packed_copy;

    // SYN038-GAP-TY-fixed_array_integral__HC-interface: complete fixed array.
    nibbles_t array_source = '{0: 4'h3, 1: 4'hc};
    nibbles_t array_copy;

    always_comb begin
        enum_copy = enum_source;
        packed_copy = packed_source;
        array_copy = array_source;
    end
endinterface

module tb;
    import typed_scope_remainder_types::*;

    typed_scope_sources_if typed_if();

    for (genvar g = 0; g < 1; g++) begin : generated_sources
        // SYN038-GAP-TY-untagged_packed_union__HC-generate: whole union value.
        union_t union_source = union_t'(8'h5a);
        union_t union_copy;

        // SYN038-GAP-TY-unpacked_record__HC-generate: whole record value.
        record_t record_source = '{tag: 4'h6, data: 4'hd};
        record_t record_copy;

        always_comb begin
            union_copy = union_source;
            record_copy = record_source;
        end
    end

    initial begin
        #1;
        if (typed_if.enum_copy !== PHASE_RUN)
            $fatal(1, "TY-enum/HC-interface mismatch");
        if (typed_if.packed_copy !== packed_t'({4'h2, 4'hb}))
            $fatal(1, "TY-packed_struct/HC-interface mismatch");
        if (typed_if.array_copy[0] !== 4'h3 || typed_if.array_copy[1] !== 4'hc)
            $fatal(1, "TY-fixed_array_integral/HC-interface mismatch");
        if (generated_sources[0].union_copy !== union_t'(8'h5a))
            $fatal(1, "TY-untagged_packed_union/HC-generate mismatch");
        if (generated_sources[0].record_copy.tag !== 4'h6 ||
            generated_sources[0].record_copy.data !== 4'hd)
            $fatal(1, "TY-unpacked_record/HC-generate mismatch");

        $display("tyhc=%h,%h,%h,%h,%h", typed_if.enum_copy,
                 typed_if.packed_copy, typed_if.array_copy[1],
                 generated_sources[0].union_copy,
                 generated_sources[0].record_copy.data);
        $finish(0);
    end
endmodule
