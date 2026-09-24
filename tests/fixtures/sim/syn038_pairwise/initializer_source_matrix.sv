// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/initializer_source_matrix.sv
interface init_sources;
    bit [7:0] member_value;
endinterface

module tb;
    function automatic logic [7:0] from_automatic_local(input logic [7:0] seed);
        automatic logic [7:0] source_value = seed + 8'h01;
        return source_value;
    endfunction

    function automatic logic [7:0] from_formal(input logic [7:0] formal_value);
        return formal_value;
    endfunction

    function automatic logic [7:0] from_return_slot(input logic [7:0] seed);
        from_return_slot = seed + 8'h01;
    endfunction

    function logic [7:0] from_static_local();
        static bit [7:0] source_value;
        source_value = source_value + 8'h01;
        return source_value;
    endfunction

    init_sources bus();

    localparam logic [7:0] AUTO_CONSTANT = from_automatic_local(8'h10);
    localparam logic [7:0] FORMAL_CONSTANT = from_formal(8'h52);
    localparam logic [7:0] RETURN_CONSTANT = from_return_slot(8'h83);
    localparam logic [7:0] STATIC_CONSTANT = from_static_local();

    logic [7:0] auto_runtime = from_automatic_local(8'h20);
    logic [7:0] formal_runtime = from_formal(8'h62);
    logic [7:0] return_runtime = from_return_slot(8'h93);
    logic [7:0] static_runtime = from_static_local();

    initial begin : check
        static logic [7:0] static_from_automatic = from_automatic_local(8'h30);
        automatic logic [7:0] automatic_from_automatic = from_automatic_local(8'h40);

        static logic [7:0] static_from_formal = from_formal(8'h72);
        static logic [7:0] static_from_return = from_return_slot(8'hA3);
        automatic logic [7:0] automatic_from_return = from_return_slot(8'hB3);

        automatic logic [7:0] automatic_from_static = from_static_local();

        static logic [7:0] static_from_interface = bus.member_value;
        automatic logic [7:0] automatic_from_interface = bus.member_value;

        // Both declaration initializers capture the two-state default before this write.
        bus.member_value = 8'hD5;

        if (AUTO_CONSTANT !== 8'h11 || auto_runtime !== 8'h21 ||
            static_from_automatic !== 8'h31 || automatic_from_automatic !== 8'h41 ||
            FORMAL_CONSTANT !== 8'h52 || formal_runtime !== 8'h62 ||
            static_from_formal !== 8'h72 || RETURN_CONSTANT !== 8'h84 ||
            return_runtime !== 8'h94 || static_from_return !== 8'hA4 ||
            automatic_from_return !== 8'hB4 || STATIC_CONSTANT !== 8'h01 ||
            static_runtime !== 8'h01 ||
            automatic_from_static !== 8'h02 || static_from_interface !== 8'h00 ||
            automatic_from_interface !== 8'h00 || bus.member_value !== 8'hD5)
            $fatal(1, "initializer source matrix mismatch");

        $display("automatic=%h,%h,%h,%h formal=%h,%h,%h return=%h,%h,%h,%h static=%h,%h,%h interface=%h,%h,%h",
                 AUTO_CONSTANT, auto_runtime,
                 static_from_automatic, automatic_from_automatic,
                 FORMAL_CONSTANT, formal_runtime, static_from_formal,
                 RETURN_CONSTANT, return_runtime, static_from_return, automatic_from_return,
                 STATIC_CONSTANT, static_runtime, automatic_from_static,
                 static_from_interface, automatic_from_interface, bus.member_value);
        $finish(0);
    end
endmodule
