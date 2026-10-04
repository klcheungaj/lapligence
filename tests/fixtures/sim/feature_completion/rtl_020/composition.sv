// IEEE 1800-2009 29.3-29.4, 29.8, 28.3.6: an 8-bit ripple-carry adder built
// from UDP instance arrays. Each carry input is a concatenation that feeds
// back a part-select of the array's own output vector. Two module instances
// share the definitions and swap their operands; an X or Z input propagates
// only as far as the majority table cannot resolve it.
primitive parity3(y, a, b, c);
    output y;
    input a, b, c;
    table
        0 0 0 : 0;
        0 0 1 : 1;
        0 1 0 : 1;
        0 1 1 : 0;
        1 0 0 : 1;
        1 0 1 : 0;
        1 1 0 : 0;
        1 1 1 : 1;
    endtable
endprimitive

// Overlapping rows agree, so the table is legal.
primitive majority3(y, a, b, c);
    output y;
    input a, b, c;
    table
        1 1 ? : 1;
        1 ? 1 : 1;
        ? 1 1 : 1;
        0 0 ? : 0;
        0 ? 0 : 0;
        ? 0 0 : 0;
    endtable
endprimitive

module adder8(input wire [7:0] a, b, input wire cin, output wire [7:0] sum,
              output wire cout);
    wire [7:0] carry;
    parity3 bits[7:0] (sum, a, b, {carry[6:0], cin});
    majority3 carries[7:0] (carry, a, b, {carry[6:0], cin});
    assign cout = carry[7];
endmodule

module tb;
    logic [7:0] a, b;
    logic cin;
    wire [7:0] s1, s2;
    wire c1, c2;

    adder8 forward(a, b, cin, s1, c1);
    adder8 swapped(b, a, 1'b0, s2, c2);

    initial begin
        a = 8'd100; b = 8'd27; cin = 1'b0;
        #1 $display("%b %b %b forward=%b %b swapped=%b %b", a, b, cin, s1, c1, s2, c2);
        a = 8'd200; b = 8'd100; cin = 1'b0;
        #1 $display("%b %b %b forward=%b %b swapped=%b %b", a, b, cin, s1, c1, s2, c2);
        a = 8'd255; b = 8'd0; cin = 1'b1;
        #1 $display("%b %b %b forward=%b %b swapped=%b %b", a, b, cin, s1, c1, s2, c2);
        a = 8'b0000_000x; b = 8'd1; cin = 1'b0;
        #1 $display("%b %b %b forward=%b %b swapped=%b %b", a, b, cin, s1, c1, s2, c2);
        a = 8'b1010_1010; b = 8'b0101_0101; cin = 1'bz;
        #1 $display("%b %b %b forward=%b %b swapped=%b %b", a, b, cin, s1, c1, s2, c2);
        $finish(0);
    end
endmodule
