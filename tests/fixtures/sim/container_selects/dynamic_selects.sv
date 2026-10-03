// llg-test-fixture: tests/fixtures/sim/container_selects/dynamic_selects.sv
// Bit, part and indexed part-select writes and reads of packed dynamic-array
// elements (IEEE 1800-2009 §7.4.6, §7.5, §11.5.1). Each write replaces only
// the selected bits of one element; an invalid element index or bit select
// writes nothing.
module tb;
    logic [129:0] d[];
    logic [0:69] asc[];
    logic [3:0][7:0] pk[];
    bit [99:0] two[];
    logic signed [69:0] s[];
    int i;
    int far;
    logic [7:0] xi;

    initial begin
        d = new[3];
        foreach (d[k]) d[k] = '0;
        d[0][129:64] = 66'h3_ffff_ffff_ffff_ffff;
        $display("d0=%h", d[0]);
        i = 1;
        d[i][0] = 1'b1;
        d[i][129] = 1'bx;
        d[i][128] = 1'bx;
        d[i][64 +: 4] = 4'bzzzz;
        $display("d1=%h", d[1]);
        d[i + 1][i * 4 +: 8] = 8'ha5;
        d[2][127 -: 4] = 4'bz10x;
        $display("d2=%h %h", d[2][129:128], d[2][123:0]);
        $display("reads=%b%b %h %h %b", d[1][129], d[1][64], d[0][129:126],
                 d[i + 1][i * 4 +: 8], d[2][127 -: 4]);

        // Invalid element index and invalid/out-of-range bit selects write
        // nothing (§7.4.6, §11.5.1).
        d[5][0] = 1'b1;
        far = 200;
        d[0][far] = 1'b0;
        xi = 8'bx;
        d[0][xi] = 1'b0;
        d[0][xi +: 2] = 2'b00;
        $display("size=%0d d0=%h", d.size(), d[0]);

        asc = new[1];
        asc[0] = '0;
        asc[0][0:3] = 4'hf;
        asc[0][66 +: 4] = 4'h9;
        $display("asc=%h", asc[0]);

        pk = new[2];
        pk[1] = '0;
        pk[1][2] = 8'h5a;
        pk[1][3][7:4] = 4'h9;
        pk[i][0][0] = 1'b1;
        $display("pk=%h byte2=%h", pk[1], pk[i][2]);

        two = new[1];
        two[0] = '1;
        two[0][99:96] = 4'bx1z0;
        $display("two=%h", two[0]);

        s = new[1];
        s[0] = '0;
        s[0][69] = 1'b1;
        $display("s=%0d", s[0]);
        $finish;
    end
endmodule
