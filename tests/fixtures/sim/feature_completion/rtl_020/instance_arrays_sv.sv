// IEEE 1800-2009 23.3.3.5, 28.3.6, 29.8: multidimensional instance arrays,
// unpacked net-array connections matched dimension by dimension, and a
// module instance array whose ports take part-selects of wider vectors.
primitive and2(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        ? 0 : 0;
        1 1 : 1;
    endtable
endprimitive

module half(input wire [1:0] i, output wire [1:0] o);
    assign o = ~i;
endmodule

module tb;
    logic [3:0] a, b;
    wire [3:0] q;
    wire y [0:3];
    wire [1:0] cells [0:1];
    wire [7:0] w;

    and2 g2[1:0][1:0] (q, a, b);
    and2 g3[0:3] (y, a, b);
    and2 g4[0:1][1:0] (cells, a, b);
    half h[1:0] (.i(a[3:0]), .o(w[5:2]));

    task automatic show(input string label);
        $display("%s q=%b y=%b%b%b%b cells=%b,%b w=%b",
                 label, q, y[0], y[1], y[2], y[3], cells[0], cells[1], w);
    endtask

    initial begin
        a = 4'b1100; b = 4'b1010;
        #1 show("A");
        a = 4'b0111; b = 4'b1101;
        #1 show("B");
        $finish(0);
    end
endmodule
