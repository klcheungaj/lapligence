// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_reduction_continuous_and_nba.sv
module tb;
    typedef logic [7:0] byte_t;

    wire [7:0] net_lanes [0:1];
    byte_t net_seed = 8'd3;
    assign net_lanes[0] = net_seed;
    assign net_lanes[1] = 8'd2;

    logic [7:0] variable_lanes [0:1];
    byte_t variable_seed = 8'd3;
    assign variable_lanes[0] = variable_seed;
    assign variable_lanes[1] = 8'd2;

    bit [7:0] nba_lanes [0:1];
    bit clk = 1'b0;

    byte_t net_sum;
    byte_t variable_sum;
    bit [7:0] nba_sum;

    always_ff @(posedge clk) begin : write_nba_lanes
        nba_lanes[0] <= 8'd3;
        nba_lanes[1] <= 8'd2;
    end

    initial begin : check
        // These vectors keep the array whose elements are written as the
        // fixed_array_reduction receiver. WK and LV apply to those same slots;
        // CO and PC describe the later reduction consumer in this process.
        // TY=fixed_array_integral, OP=direct_projection, CO=assignment_rhs,
        // LV=element, SL=module_package, FM=none, HC=module, HR=local,
        // CP=fixed_array_reduction, CT=none, IN=none, WK=continuous_net,
        // PC=initial.
        #1;
        net_sum = net_lanes.sum();
        if (net_lanes[0] !== 8'd3 || net_lanes[1] !== 8'd2 || net_sum !== 8'd5)
            $fatal(1, "continuous net reduction baseline mismatch");

        // TY=fixed_array_integral, OP=direct_projection, CO=assignment_rhs,
        // LV=element, SL=module_package, FM=none, HC=module, HR=local,
        // CP=fixed_array_reduction, CT=none, IN=none, WK=continuous_variable,
        // PC=initial.
        variable_sum = variable_lanes.sum();
        if (variable_lanes[0] !== 8'd3 || variable_lanes[1] !== 8'd2 ||
            variable_sum !== 8'd5)
            $fatal(1, "continuous variable reduction baseline mismatch");

        net_seed = 8'd4;
        variable_seed = 8'd4;
        #1;
        if (net_lanes[0] !== 8'd4 || net_lanes[1] !== 8'd2 || net_sum !== 8'd5)
            $fatal(1, "continuous net source update mismatch");
        if (variable_lanes[0] !== 8'd4 || variable_lanes[1] !== 8'd2 ||
            variable_sum !== 8'd5)
            $fatal(1, "continuous variable source update mismatch");
        net_sum = net_lanes.sum();
        variable_sum = variable_lanes.sum();
        if (net_sum !== 8'd6 || variable_sum !== 8'd6)
            $fatal(1, "continuous reduction update mismatch");

        // TY=fixed_array_integral, OP=direct_projection, CO=assignment_rhs,
        // LV=element, SL=module_package, FM=none, HC=module, HR=local,
        // CP=fixed_array_reduction, CT=none, IN=none, WK=procedural_nba,
        // PC=initial. The two-state array supplies a deterministic zero
        // baseline without introducing another procedural writer.
        clk = 1'b1;
        #0;
        nba_sum = nba_lanes.sum();
        if (nba_lanes[0] !== 8'd0 || nba_lanes[1] !== 8'd0 || nba_sum !== 8'd0)
            $fatal(1, "NBA pre-commit reduction mismatch");
        #1;
        if (nba_lanes[0] !== 8'd3 || nba_lanes[1] !== 8'd2 || nba_sum !== 8'd0)
            $fatal(1, "NBA committed source mismatch");
        nba_sum = nba_lanes.sum();
        if (nba_sum !== 8'd5)
            $fatal(1, "NBA post-commit reduction mismatch");

        $display("net-reduction=%0d", net_sum);
        $display("var-reduction=%0d", variable_sum);
        $display("nba-reduction=%0d", nba_sum);
        $finish(0);
    end
endmodule
