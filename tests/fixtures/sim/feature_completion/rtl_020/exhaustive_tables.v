// IEEE 1364-2001 8.1.4-8.1.6, 8.2; IEEE 1800-2009 29.3-29.4.
// Every combination of 0/1/x/z on three shared inputs drives a mux, a parity
// table and a table using every combinational symbol. The sweep runs forward
// and then backward, so each combination is also reached from other states.
primitive mux3(y, sel, a, b);
    output y;
    input sel, a, b;
    table
    //  sel a b : y
        0   0 ? : 0;
        0   1 ? : 1;
        1   ? 0 : 0;
        1   ? 1 : 1;
        x   0 0 : 0;
        x   1 1 : 1;
    endtable
endprimitive

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

// Inputs use 0 1 x X b B ?; outputs use 0 1 x X. The second row overlaps
// the first with the same output, which is legal.
primitive symbols(y, a, b, c);
    output y;
    input a, b, c;
    table
        0 b ? : 0;
        0 B 0 : 0;
        1 0 ? : 1;
        1 1 b : X;
        1 X ? : 1;
        x 0 0 : 1;
        X 1 B : 0;
        x x ? : 0;
        x 0 1 : x;
    endtable
endprimitive

module tb;
    reg [3:0] values;
    reg i0, i1, i2;
    wire m, p, s;
    integer a, b, c, pass;

    mux3 mux(m, i0, i1, i2);
    parity3 parity(p, i0, i1, i2);
    symbols table_symbols(s, i0, i1, i2);

    initial begin
        values = 4'bzx10;
        for (pass = 0; pass < 2; pass = pass + 1)
            for (a = 0; a < 4; a = a + 1)
                for (b = 0; b < 4; b = b + 1)
                    for (c = 0; c < 4; c = c + 1) begin
                        i0 = values[pass == 0 ? a : 3 - a];
                        i1 = values[pass == 0 ? b : 3 - b];
                        i2 = values[pass == 0 ? c : 3 - c];
                        #1 $display("%b%b%b m=%b p=%b s=%b", i0, i1, i2, m, p, s);
                    end
        $finish(0);
    end
endmodule
