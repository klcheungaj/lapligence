// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/dependent_types.sv
// IEEE 1800-2009 6.20.3, 6.23, 13.4.3, 25.8, 26.4 and 26.6.
// Equal-width but distinct enums must not share nominal type specialization.
package dependent_base;
    parameter int BIAS = 3;
    typedef enum logic signed [3:0] {E_NEG = -2, E_POS = 3} enum_t;
    typedef enum logic signed [3:0] {F_NEG = -2, F_POS = 3} other_t;
    function automatic int factorial(input int n);
        if (n <= 1) return 1;
        return n * factorial(n - 1);
    endfunction
    function automatic int add_bias(input int n, input int bias = BIAS);
        return n + bias;
    endfunction
    let plus_bias(x) = x + BIAS;
endpackage

package dependent_export;
    import dependent_base::*;
    export dependent_base::*;
endpackage

interface dependent_bus #(parameter type T = logic [3:0]);
    T data;
endinterface

module dependent_cell import dependent_export::*; #(
    parameter type T = enum_t,
    parameter T RESET = T'(E_NEG),
    parameter int WIDTH = $bits(T)
) (
    dependent_bus bus,
    output T value,
    output T reset_value,
    output logic signed [WIDTH:0] widened,
    output int signature,
    output bit is_original_enum
);
    typedef T alias_t;
    typedef type(value) value_t;
    localparam bit SAME = type(alias_t) == type(value_t);
    localparam int FACT = factorial(3);
    localparam int DEFAULTED = add_bias(2);
    localparam string DECIMAL = "12";
    localparam int STRING_WIDTH = DECIMAL.atoi();
    localparam int REAL_WIDTH = int'($sqrt(81.0));
    localparam int LEFT = $left(alias_t);
    localparam int RIGHT = $right(alias_t);
    logic [STRING_WIDTH-1:0] string_sized;
    logic [REAL_WIDTH-1:0] real_sized;
    assign value = bus.data;
    assign reset_value = RESET;
    assign widened = value;
    assign signature = FACT + DEFAULTED + $bits(string_sized) + $bits(real_sized)
                     + LEFT + RIGHT + int'(SAME);
    generate
        if (type(T) == type(enum_t)) begin : original_enum
            assign is_original_enum = 1;
        end else begin : other_type
            assign is_original_enum = 0;
        end
        for (genvar g = 0; g < 2; g = g + 1) begin : lanes
            localparam int WIDTH = g + 1;
            wire [WIDTH-1:0] mask;
            assign mask = {WIDTH{1'b1}};
        end
    endgenerate
endmodule

module tb import dependent_export::*; ();
    dependent_bus #(enum_t) bus0();
    dependent_bus #(logic [6:0]) bus1();
    dependent_bus #(other_t) bus2();
    enum_t value0, reset0;
    logic [6:0] value1, reset1;
    other_t value2, reset2;
    logic signed [4:0] wide0, wide2;
    logic signed [7:0] wide1;
    int sig0, sig1, sig2;
    bit original0, original1, original2;
    dependent_cell c0(bus0, value0, reset0, wide0, sig0, original0);
    dependent_cell #(.T(logic [6:0])) c1(bus1, value1, reset1, wide1, sig1, original1);
    dependent_cell #(.T(other_t)) c2(bus2, value2, reset2, wide2, sig2, original2);
    initial begin
        bus0.data = E_NEG;
        bus1.data = 7'h45;
        bus2.data = F_NEG;
        #1;
        $display("types=%0d/%0d/%0d reset=%h/%h/%h sig=%0d/%0d/%0d nominal=%0d%0d%0d masks=%h/%h let=%0d",
                 wide0, wide1, wide2, reset0, reset1, reset2, sig0, sig1, sig2,
                 original0, original1, original2,
                 c0.lanes[0].mask, c1.lanes[1].mask, plus_bias(4));
        $finish(0);
    end
endmodule
