// IEEE 1800-2009 6.19: an enum has the state domain and signedness of its
// base type. The default int base is two-state signed; a logic base is
// four-state and unsigned unless explicitly tagged signed.
module tb #(parameter WIDTH = 128);
    typedef enum int {INT_ONE = 1, INT_TWO = 2} int_enum_t;
    typedef enum int unsigned {UINT_ONE = 1, UINT_TWO = 2} uint_enum_t;
    typedef enum logic [3:0] {LOGIC_ONE = 1, LOGIC_TWO = 2} logic_enum_t;
    typedef enum logic signed [3:0] {SIGNED_ONE = 1, SIGNED_TWO = 2}
        signed_logic_enum_t;

    int_enum_t int_value;
    uint_enum_t uint_value;
    logic_enum_t logic_value;
    signed_logic_enum_t signed_logic_value;
    logic [31:0] source32;
    logic [3:0] source4;
    logic [WIDTH-1:0] observer;
    logic [WIDTH-1:0] expected;
    integer failed;

    initial begin
        failed = 0;
        if (int_value !== 0 || uint_value !== 0 || logic_value !== 4'hx ||
            signed_logic_value !== 4'hx) begin
            $display("FAIL enum-defaults WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source32 = 32'h8000_0007;
        source32[0] = 1'bx;
        source32[1] = 1'bz;
        int_value = int_enum_t'(source32);
        observer = int_value;
        expected = '1;
        expected[30:0] = 31'h0000_0004;
        if (!failed && observer !== expected) begin
            $display("FAIL int-enum-cast-and-sign-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        uint_value = uint_enum_t'(32'h8000_0001);
        observer = uint_value;
        expected = '0;
        expected[31:0] = 32'h8000_0001;
        if (!failed && observer !== expected) begin
            $display("FAIL unsigned-int-enum-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source4 = 4'b1xz0;
        logic_value = logic_enum_t'(source4);
        observer = logic_value;
        expected = '0;
        expected[3:0] = 4'b1xz0;
        if (!failed && observer !== expected) begin
            $display("FAIL logic-enum-four-state-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_logic_value = signed_logic_enum_t'(4'b1001);
        observer = signed_logic_value;
        expected = '1;
        expected[3:0] = 4'b1001;
        if (!failed && observer !== expected) begin
            $display("FAIL signed-logic-enum-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS enum_state_defaults WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
