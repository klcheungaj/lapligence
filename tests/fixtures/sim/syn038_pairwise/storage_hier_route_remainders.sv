// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/storage_hier_route_remainders.sv
// IEEE 1800-2009 §§6.21, 10.3.1, 10.3.2, 13.5.2, 23.3, 23.6, and 25.2:
// static/automatic block storage, qualified hierarchy, interface member routes,
// static continuous assignments, continuous function results, and ref forwarding.

interface storage_route_nested_if;
    logic [7:0] member_value;
endinterface

interface storage_route_if;
    storage_route_nested_if nested();
    wire net_member;
    logic variable_member;
    logic [7:0] static_seen;
    logic [7:0] automatic_seen;
    logic [7:0] return_seen;
    logic [7:0] formal_seen;
    logic [7:0] primary_seen;
    logic [7:0] sibling_seen;

    task static capture_primary(input logic [7:0] value);
        primary_seen = value;
    endtask

    task static capture_sibling(input logic [7:0] value);
        sibling_seen = value;
    endtask

    initial begin : interface_process
        // SYN038-GAP-SL-static_local__HC-interface; focal storage is this local.
        static logic [7:0] static_value;
        // SYN038-GAP-SL-automatic_local__HC-interface; focal storage is this local.
        automatic logic [7:0] automatic_value;
        static_value = 8'h31;
        automatic_value = 8'h42;
        static_seen = static_value;
        automatic_seen = automatic_value;
        // SYN038-GAP-SL-return_slot__HC-interface; focal source is local_result's result.
        return_seen = local_result();
        // A function result initializes a distinct nested interface member.
        nested.member_value = local_result();
        capture_primary(8'h66);
        capture_sibling(8'h39);
        // SYN038-GAP-SL-formal__HC-interface; sibling task has a same-named formal.
        formal_seen = capture_primary.value;
        if (formal_seen !== 8'h66 || capture_sibling.value !== 8'h39 ||
            primary_seen !== 8'h66 || sibling_seen !== 8'h39 ||
            nested.member_value !== 8'h53)
            $fatal(1, "interface static formal hierarchy mismatch: %h %h",
                   formal_seen, capture_sibling.value);
    end

    function static logic [7:0] local_result;
        local_result = 8'h53;
        return local_result;
    endfunction
endinterface

module storage_route_child;
    wire net_target;
    logic variable_target;

    task automatic bump(ref logic [7:0] value);
        value = value ^ 8'h11;
    endtask
endmodule

module tb;
    logic [7:0] source;
    logic [7:0] function_variable;
    logic [7:0] value;
    storage_route_if bus();
    storage_route_child worker();

    function automatic logic [7:0] make_value(input logic [7:0] input_value);
        return input_value ^ 8'h11;
    endfunction

    // SYN038-GAP-HR-hierarchical_identifier__WK-continuous_net.
    assign worker.net_target = source[0];
    // Child hierarchy continuous-variable control alongside the net route.
    assign worker.variable_target = source[1];
    // SYN038-GAP-HR-interface_member__WK-continuous_net.
    assign bus.net_member = source[2];
    // SYN038-GAP-HR-interface_member__WK-continuous_variable.
    assign bus.variable_member = source[3];
    // The function result source and continuous destination are separate slots.
    assign function_variable = make_value(source);

    generate if (1) begin : generated
        logic [7:0] static_seen;
        logic [7:0] automatic_seen;

        initial begin : generated_process
            // SYN038-GAP-SL-static_local__HC-generate; focal storage is this local.
            static logic [7:0] static_value;
            // SYN038-GAP-SL-automatic_local__HC-generate; focal storage is this local.
            automatic logic [7:0] automatic_value;
            static_value = 8'h64;
            automatic_value = 8'h75;
            static_seen = static_value;
            automatic_seen = automatic_value;
        end
    end endgenerate

    task automatic use_automatic_local;
        automatic logic [7:0] local_value;
        local_value = 8'h12;
        // SYN038-GAP-SL-automatic_local__HR-hierarchical_identifier.
        worker.bump(local_value);
        if (local_value !== 8'h03)
            $fatal(1, "hierarchical automatic-local ref mismatch: %h", local_value);
    endtask

    task automatic use_formal(ref logic [7:0] formal_value);
        // SYN038-GAP-SL-formal__HR-hierarchical_identifier.
        worker.bump(formal_value);
    endtask

    initial begin
        source = 8'h00;
        use_automatic_local();
        value = 8'h23;
        use_formal(value);
        if (value !== 8'h32)
            $fatal(1, "hierarchical formal ref mismatch: %h", value);

        #1;
        if (bus.static_seen !== 8'h31 || bus.automatic_seen !== 8'h42 ||
            bus.return_seen !== 8'h53 || bus.nested.member_value !== 8'h53)
            $fatal(1, "interface local scope mismatch");
        if (generated.static_seen !== 8'h64 || generated.automatic_seen !== 8'h75)
            $fatal(1, "generate local scope mismatch");
        if (worker.net_target !== 1'b0 || worker.variable_target !== 1'b0 ||
            bus.net_member !== 1'b0 || bus.variable_member !== 1'b0 ||
            function_variable !== 8'h11)
            $fatal(1, "hierarchical write mismatch at zero");

        source = 8'h0f;
        #1;
        if (worker.net_target !== 1'b1 || worker.variable_target !== 1'b1 ||
            bus.net_member !== 1'b1 || bus.variable_member !== 1'b1 ||
            function_variable !== 8'h1e)
            $fatal(1, "hierarchical write mismatch after transition");
        $display("storage_hier_route_remainders=passed");
        $finish;
    end
endmodule
