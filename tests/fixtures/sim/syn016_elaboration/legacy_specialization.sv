// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/legacy_specialization.sv
// IEEE 1364-2001 3.3.1, 10.3.5 and 12.2.1: legal negative index
// labels, independent constant recursion, and defparam specialization.
module legacy_cell #(
    parameter integer WIDTH = 4,
    parameter integer SHIFT = 1
) (
    input [WIDTH-1:0] data,
    output [WIDTH-1:0] value,
    output signed [WIDTH:0] widened
);
    function automatic integer factorial(input integer n);
        if (n <= 1) factorial = 1;
        else factorial = n * factorial(n - 1);
    endfunction
    // No defparam-affected parameter is read by the constant function:
    // the supplied 2001/2009 references leave that case undefined.
    localparam integer FACT = factorial(3);
    localparam integer DEPENDENT = WIDTH + SHIFT;
    localparam real SCALE = 1.5;
    localparam integer ROUNDED = SCALE * 3.0;
    wire [1-WIDTH:0] ascending;
    wire [FACT-1:0] folded;
    assign ascending = data;
    assign folded = FACT;
    assign widened = $signed(data);
    genvar lane_index;
    generate
        for (lane_index = 0; lane_index < WIDTH; lane_index = lane_index + 1) begin : lanes
            localparam integer INDEX = lane_index;
            assign value[INDEX] = ascending[-INDEX];
        end
    endgenerate
endmodule

module tb;
    reg [3:0] data0;
    reg [6:0] data1;
    wire [3:0] value0;
    wire [6:0] value1;
    wire signed [4:0] widened0;
    wire signed [7:0] widened1;
    legacy_cell c0(data0, value0, widened0);
    legacy_cell c1(data1, value1, widened1);
    defparam c1.WIDTH = 7;
    defparam c1.SHIFT = 2;
    initial begin
        data0 = 4'hf;
        data1 = 7'h45;
        #1;
        $display("legacy=%h/%h signed=%0d/%0d dependent=%0d/%0d factorial=%0d/%0d rounded=%0d/%0d",
                 value0, value1, widened0, widened1, c0.DEPENDENT, c1.DEPENDENT,
                 c0.FACT, c1.FACT, c0.ROUNDED, c1.ROUNDED);
        $finish(0);
    end
endmodule
