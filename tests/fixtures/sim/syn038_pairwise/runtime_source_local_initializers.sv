// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/runtime_source_local_initializers.sv
// IEEE 1800-2009 §§6.8 and 10.5: module runtime variables are consumed by
// static and automatic block-variable declaration initializers.
module tb;
    typedef enum logic [7:0] { RED = 8'h21, BLUE = 8'h5C } color_t;
    typedef logic [7:0] byte_pair_t [0:1];

    logic [7:0] runtime_scalar = 8'h39;
    color_t runtime_color = BLUE;
    byte_pair_t runtime_lanes = '{8'h39, 8'h4A};

    initial begin : check
        // Each RHS below is a distinct focal read from its module source.
        // runtime_scalar -> static_scalar: TY=integral_bit_logic OP=direct_projection
        // CO=declaration_initializer LV=none SL=module_package FM=none HC=module
        // HR=local CP=none CT=none IN=static_local WK=none PC=none.
        static logic [7:0] static_scalar = runtime_scalar;
        // runtime_scalar -> automatic_scalar has IN=automatic_local, PC=initial;
        // all other factors match the static_scalar observation.
        automatic logic [7:0] automatic_scalar = runtime_scalar;

        // runtime_color -> static_color: TY=enum; runtime_color -> automatic_color
        // differs only by IN=automatic_local, PC=initial. Both otherwise use
        // OP=direct_projection CO=declaration_initializer LV=none
        // SL=module_package FM=none HC=module HR=local CP=none CT=none WK=none.
        static color_t static_color = runtime_color;
        automatic color_t automatic_color = runtime_color;

        // runtime_lanes -> static_lanes: TY=fixed_array_integral; the automatic
        // consumer differs only by IN=automatic_local, PC=initial. Both otherwise
        // use OP=direct_projection CO=declaration_initializer LV=none
        // SL=module_package FM=none HC=module HR=local CP=none CT=none WK=none.
        static byte_pair_t static_lanes = runtime_lanes;
        automatic byte_pair_t automatic_lanes = runtime_lanes;

        if (static_scalar !== 8'h39 || automatic_scalar !== 8'h39 ||
            static_color !== BLUE || automatic_color !== BLUE ||
            static_lanes[0] !== 8'h39 || static_lanes[1] !== 8'h4A ||
            automatic_lanes[0] !== 8'h39 || automatic_lanes[1] !== 8'h4A)
            $fatal(1, "runtime source local initializer mismatch");

        $display("runtime-local-init=%h,%h color=%h,%h lanes=%h,%h:%h,%h",
                 static_scalar, automatic_scalar, static_color, automatic_color,
                 static_lanes[0], static_lanes[1],
                 automatic_lanes[0], automatic_lanes[1]);
    end
endmodule
