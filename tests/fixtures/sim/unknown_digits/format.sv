// IEEE 1800-2009 21.2.1.4: %h/%o digits and %d values whose bits are all x/z
// print lowercase; partially unknown ones print uppercase (X before Z). %b
// always prints per-bit lowercase. Expected lines are fixed by the test.
`define SHOW(tag, v) $display("%s %h|%o|%0d|%b", tag, v, v, v, v);

module tb;
    logic w1;
    logic [3:0] w4;
    logic [7:0] w8;
    logic [64:0] w65;
    logic [129:0] w130;
    string s;

    initial begin
        w1 = 1'bx; `SHOW("w1.x", w1)
        w1 = 1'bz; `SHOW("w1.z", w1)
        w1 = 1'b1; `SHOW("w1.1", w1)

        w4 = 4'bxxxx; `SHOW("w4.xxxx", w4)
        w4 = 4'bzzzz; `SHOW("w4.zzzz", w4)
        w4 = 4'b0x00; `SHOW("w4.0x00", w4)
        w4 = 4'b0z00; `SHOW("w4.0z00", w4)
        w4 = 4'bxz01; `SHOW("w4.xz01", w4)
        w4 = 4'bzz01; `SHOW("w4.zz01", w4)
        w4 = 4'bxxzz; `SHOW("w4.xxzz", w4)
        w4 = 4'b1010; `SHOW("w4.1010", w4)

        w8 = 8'bxxxxxxxx; `SHOW("w8.all_x", w8)
        w8 = 8'bzzzzzzzz; `SHOW("w8.all_z", w8)
        w8 = 8'b0000x000; `SHOW("w8.part_x", w8)
        w8 = 8'b0z000000; `SHOW("w8.part_z", w8)
        w8 = 8'bxxxxzzzz; `SHOW("w8.xxxxzzzz", w8)
        w8 = 8'bzzzz0101; `SHOW("w8.zzzz0101", w8)
        w8 = 8'h5a; `SHOW("w8.5a", w8)

        w65 = {65{1'bx}}; `SHOW("w65.all_x", w65)
        w65 = {65{1'bz}}; `SHOW("w65.all_z", w65)
        w65 = 65'b0; w65[64] = 1'bx; `SHOW("w65.top_x", w65)
        w65 = 65'b0; w65[64] = 1'b1; w65[0] = 1'bz; `SHOW("w65.top1_low_z", w65)
        w65 = 65'b0; w65[64] = 1'bz; w65[63] = 1'bx; `SHOW("w65.straddle", w65)

        w130 = {130{1'bx}}; `SHOW("w130.all_x", w130)
        w130 = {130{1'bz}}; `SHOW("w130.all_z", w130)
        w130 = 130'b0; w130[129:128] = 2'bxx; `SHOW("w130.top_xx", w130)
        w130 = 130'b0; w130[129] = 1'bz; w130[128] = 1'b1; `SHOW("w130.top_z1", w130)
        w130 = 130'b0; w130[129] = 1'b1; w130[70] = 1'bx; w130[5] = 1'bz;
        `SHOW("w130.mixed", w130)

        w8 = 8'b0000x000;
        s = $sformatf("%h|%x|%H|%o|%0d|%b", w8, w8, w8, w8, w8, w8);
        $display("sformatf.w8 %s", s);
        w4 = 4'bzz01;
        s = $sformatf("%h|%o|%0d|%b", w4, w4, w4, w4);
        $display("sformatf.w4 %s", s);
        w130 = 130'b0; w130[129] = 1'bz; w130[128] = 1'b1;
        s = $sformatf("%h|%o|%0d", w130, w130, w130);
        $display("sformatf.w130 %s", s);
    end
endmodule
