// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/scope_hierarchy_matrix.sv
package scope_hierarchy_types;
    typedef enum logic [7:0] { SLEEP = 8'h21, WAKE = 8'h5c } phase_t;
    typedef bit [7:0] byte_t;
    typedef logic [7:0] bytes_t [0:1];
    typedef struct {
        logic [7:0] key;
        logic [7:0] payload;
    } record_t;
    typedef record_t records_t [0:1];

endpackage

interface scope_member_if;
    bit [7:0] member_value;
endinterface

module scope_leaf #(
    parameter logic [7:0] LIMIT = 8'h10
);
    logic [7:0] value = LIMIT;
    bit [7:0] default_value;
endmodule

interface scope_source_if;
    import scope_hierarchy_types::*;

    scope_member_if nested();
    logic [7:0] member_value = 8'ha7;
    bit [7:0] default_member;
    logic [7:0] cast_source = 8'hx5;
    phase_t member_phase = WAKE;
    bytes_t member_bytes = '{8'h11, 8'h22};
    record_t member_record = '{key: 8'h33, payload: 8'h44};
    records_t member_records = '{
        0: '{key: 8'h55, payload: 8'h66},
        1: '{key: 8'h77, payload: 8'h88}
    };
    logic [7:0] event_value = 8'h00;

    logic [7:0] interface_hierarchy_read;
    logic [7:0] interface_member_read;

    always_comb interface_hierarchy_read = $root.tb.child.value;
    always_comb interface_member_read = nested.member_value;

    initial begin
        #2 event_value = 8'h01;
    end
endinterface

module tb;
    import scope_hierarchy_types::*;

    scope_source_if bus();
    scope_leaf #(.LIMIT(8'h10)) child();

    // Same-spelled module storage distinguishes bus.member_value from a local.
    logic [7:0] member_value = 8'h55;
    logic [7:0] enum_copy;
    bytes_t integral_array_copy;
    records_t record_array_copy;
    record_t unpacked_record_copy;
    logic [7:0] conditional_copy;
    logic equality_copy;
    logic [7:0] cast_copy;
    logic [7:0] pattern_copy;
    logic select_source = 1'b1;
    logic [7:0] function_return_copy;
    logic [31:0] initializer_copy;
    logic [7:0] nested_member_initial;

    logic [7:0] module_comb_hierarchical;
    logic [7:0] module_latch_interface;
    logic [7:0] module_latch_hierarchical;
    logic [7:0] generated_hierarchical [0:0];
    logic [7:0] generated_interface [0:0];
    logic interface_latch_enable;
    logic hierarchy_latch_enable;
    logic clk = 1'b0;
    logic [7:0] ff_hierarchical;
    logic event_armed = 1'b0;
    int interface_event_count = 0;

    function automatic logic [7:0] read_interface_member;
        return bus.member_value;
    endfunction

    function automatic logic [31:0] read_source_initializers;
        automatic logic [7:0] automatic_hierarchical = child.value;
        static logic [7:0] static_hierarchical = child.default_value;
        automatic logic [7:0] automatic_interface = bus.member_value;
        static logic [7:0] static_interface = bus.default_member;
        return {
            automatic_hierarchical,
            static_hierarchical,
            automatic_interface,
            static_interface
        };
    endfunction

    // HC=module, HR=hierarchical_identifier, PC=always_comb.
    always_comb module_comb_hierarchical = child.value;

    // HC=module, HR=interface_member, PC=always_latch.
    always_latch begin
        if (interface_latch_enable)
            module_latch_interface = bus.member_value;
    end

    // HC=module, HR=hierarchical_identifier, PC=always_latch.
    always_latch begin
        if (hierarchy_latch_enable)
            module_latch_hierarchical = child.value;
    end

    // HC=module, HR=hierarchical_identifier, PC=always_ff.
    always_ff @(posedge clk)
        ff_hierarchical <= child.value;

    // HC=module, HR=interface_member, PC=always.
    always @(bus.event_value) begin
        if (event_armed)
            interface_event_count++;
    end

    // The generated use sites have HC=generate. Child qualification wins for
    // the first source; the concrete interface member selects interface_member
    // for the second.
    for (genvar g = 0; g < 1; g++) begin : generated
        scope_leaf #(.LIMIT(8'h20)) generated_child();
        always_comb begin
            generated_hierarchical[g] = generated_child.value;
            generated_interface[g] = bus.member_value;
        end
    end

    initial begin
        interface_latch_enable = 1'b0;
        hierarchy_latch_enable = 1'b0;
        #1;
        nested_member_initial = bus.interface_member_read;
        enum_copy = bus.member_phase;
        integral_array_copy = bus.member_bytes;
        record_array_copy = bus.member_records;
        unpacked_record_copy = bus.member_record;
        conditional_copy = select_source ? bus.member_value : 8'h00;
        equality_copy = bus.member_value == member_value;
        cast_copy = byte_t'(bus.cast_source);
        pattern_copy = '{
            bus.member_value[0], bus.member_value[1],
            bus.member_value[2], bus.member_value[3],
            bus.member_value[4], bus.member_value[5],
            bus.member_value[6], bus.member_value[7]
        };
        function_return_copy = read_interface_member();
        initializer_copy = read_source_initializers();
        child.default_value = 8'h2c;
        bus.default_member = 8'h3d;
        bus.nested.member_value = 8'h6b;
        #0;

        if (member_value !== 8'h55 || nested_member_initial !== 8'h00 ||
            bus.interface_member_read !== 8'h6b ||
            child.default_value !== 8'h2c || bus.default_member !== 8'h3d ||
            bus.nested.member_value !== 8'h6b ||
            enum_copy !== WAKE ||
            integral_array_copy[0] !== 8'h11 || integral_array_copy[1] !== 8'h22 ||
            record_array_copy[1].payload !== 8'h88 ||
            unpacked_record_copy.payload !== 8'h44)
            $fatal(1, "interface source type routes mismatch");
        if (conditional_copy !== 8'ha7 || equality_copy !== 1'b0 ||
            cast_copy !== 8'h05 || pattern_copy !== 8'he5 ||
            function_return_copy !== 8'ha7)
            $fatal(1, "interface source operation or return mismatch");
        if (initializer_copy !== {8'h10, 8'h00, 8'ha7, 8'h00})
            $fatal(1, "hierarchical/interface declaration initializer mismatch");
        if (bus.interface_hierarchy_read !== 8'h10 || bus.interface_member_read !== 8'h6b ||
            generated_hierarchical[0] !== 8'h20 || generated_interface[0] !== 8'ha7 ||
            module_comb_hierarchical !== 8'h10)
            $fatal(1, "scoped hierarchy source routes mismatch");

        event_armed = 1'b1;
        interface_latch_enable = 1'b1;
        hierarchy_latch_enable = 1'b1;
        #1;
        if (module_latch_interface !== 8'ha7 || module_latch_hierarchical !== 8'h10)
            $fatal(1, "hierarchical/interface latch source mismatch");
        clk = 1'b1;
        #1;
        if (ff_hierarchical !== 8'h10)
            $fatal(1, "hierarchical flip-flop source mismatch");
        #1;
        if (interface_event_count !== 1)
            $fatal(1, "interface event source mismatch");

        $display("scope=if:%h,%h,%h gen:%h,%h module:%h types:%h,%h,%h,%h op:%h,%b,%h,%h return:%h init:%h event=%0d latch=%h,%h ff=%h",
                 bus.interface_hierarchy_read, bus.interface_member_read,
                 nested_member_initial, generated_hierarchical[0],
                 generated_interface[0],
                 module_comb_hierarchical, enum_copy, integral_array_copy[1],
                 record_array_copy[1].payload, unpacked_record_copy.payload,
                 conditional_copy, equality_copy, cast_copy, pattern_copy,
                 function_return_copy, initializer_copy, interface_event_count,
                 module_latch_interface, module_latch_hierarchical, ff_hierarchical);
        $finish(0);
    end
endmodule
