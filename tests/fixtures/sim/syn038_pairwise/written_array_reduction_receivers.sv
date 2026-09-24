// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_array_reduction_receivers.sv
module tb;
    typedef logic [7:0] byte_t;
    typedef byte_t byte_array_t [0:3];

    byte_array_t lanes;
    byte_t whole_sum;
    byte_t element_sum;
    byte_t row_slice_sum;
    byte_t concatenation_sum;
    byte_t positional_pattern_sum;

    initial begin : check
        // Each vector uses TY=fixed_array_integral, OP=direct_projection,
        // CO=assignment_rhs, SL=module_package, FM=none, HC=module, HR=local,
        // CP=fixed_array_reduction, CT=none, IN=none, PC=initial. LV and WK
        // below describe the earlier same-source writer.
        // Neighbor: LV=whole_object, CP=fixed_array_reduction, WK=procedural_blocking.
        lanes = '{8'h01, 8'h02, 8'h03, 8'h04};
        whole_sum = lanes.sum();

        // Each selected write targets the same outer fixed-array declaration
        // that the immediately following fixed-array reduction reads.
        // LV=element, CP=fixed_array_reduction, WK=procedural_blocking.
        lanes[1] = 8'h21;
        element_sum = lanes.sum();

        // LV=row_slice, CP=fixed_array_reduction, WK=procedural_blocking.
        lanes[0:1] = '{0:8'h10, 1:8'h20};
        row_slice_sum = lanes.sum();

        // LV=concatenation, CP=fixed_array_reduction, WK=procedural_blocking.
        {lanes[0], lanes[2]} = {8'h05, 8'h07};
        concatenation_sum = lanes.sum();

        // LV=positional_pattern, CP=fixed_array_reduction, WK=procedural_blocking.
        byte_array_t'{lanes[0], lanes[1], lanes[2], lanes[3]} = '{8'h03, 8'h06, 8'h09, 8'h0c};
        positional_pattern_sum = lanes.sum();

        if (whole_sum !== 8'h0a || element_sum !== 8'h29 ||
            row_slice_sum !== 8'h37 || concatenation_sum !== 8'h30 ||
            positional_pattern_sum !== 8'h1e)
            $fatal(1, "written array reduction receiver mismatch");

        $display("written-reduction=%02h,%02h,%02h,%02h,%02h",
                 whole_sum, element_sum, row_slice_sum, concatenation_sum,
                 positional_pattern_sum);
        $finish(0);
    end
endmodule
