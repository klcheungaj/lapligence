// llg-test-fixture: tests/fixtures/sim/array_sensitivity/edge_paths.sv
// IEEE 1800-2009 sections 7.4, 7.5, 7.10, 9.4.2, 9.4.3, 10.3 and 13.4:
// storage identity, unchanged writes, container copy and mutation-only
// sensitivity paths retain the same dependency markers after aliasing.
module tb;
    logic [7:0] fixed [0:0];
    logic [7:0] dynamic_values[];
    logic [7:0] dynamic_copy[];
    logic [7:0] queue[$];
    logic trigger;
    logic [7:0] fixed_observed;
    logic [7:0] copy_observed;
    integer value_changes;

    function automatic void set_fixed(ref logic [7:0] value);
        value = 8'h5a;
    endfunction

    always_comb fixed_observed = fixed[0];
    assign copy_observed = dynamic_copy[0];

    always @(dynamic_values[0]) begin
        value_changes = value_changes + 1;
    end

    // The queue is a write-only receiver for this @* body. An external queue
    // mutation must not wake it and cause a second push.
    always @* begin
        if (trigger)
            queue.push_back(8'h33);
    end

    initial begin
        fixed[0] = 8'h00;
        dynamic_values = new[1];
        dynamic_values[0] = 8'h11;
        dynamic_copy = new[1];
        dynamic_copy[0] = 8'h00;
        trigger = 1'b0;
        queue.push_back(8'h11);

        #1;
        value_changes = 0;
        dynamic_values[0] = 8'h11;
        #1 $display("unchanged=%0d", value_changes);

        dynamic_values[0] = 8'h22;
        #1 $display("changed=%0d", value_changes);

        set_fixed(fixed[0]);
        #1 $display("alias=%h", fixed_observed);

        dynamic_copy = dynamic_values;
        #1 $display("copy=%h", copy_observed);

        trigger = 1'b1;
        #1 $display("push=%0d", queue.size());
        queue.push_back(8'h22);
        #1 $display("lhs_only=%0d", queue.size());
        queue.delete();
        #1 $display("delete=%0d", queue.size());
        $finish;
    end
endmodule
