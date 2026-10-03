// llg-test-fixture: tests/fixtures/sim/container_selects/assoc_selects.sv
// Select writes of packed associative-array elements (IEEE 1800-2009 §7.8,
// §7.9.11, §11.5.1). A select write to a missing key creates the entry from
// the value a read of that key returns (the type default, or the array's
// specified default) and then replaces the selected bits.
module tb;
    logic [129:0] a[int];
    bit [69:0] b[int];
    logic [7:0] dflt[int] = '{default: 8'h3c};
    logic [69:0] sa[string];
    string name;
    int k;

    initial begin
        a[7] = '0;
        a[7][0] = 1'b1;
        a[7][129:128] = 2'bxz;
        $display("a7=%b %h", a[7][129:128], a[7][127:0]);
        a[-3][5:4] = 2'b10;
        $display("a-3=%h %b %h n=%0d e=%0d", a[-3][129:8], a[-3][7:0], a[-3][3:0],
                 a.num(), a.exists(-3));
        k = 4;
        a[k][k +: 8] = 8'hff;
        $display("a4=%h n=%0d", a[4], a.num());
        $display("reads=%b %b %h", a[7][0], a[7][129], a[k][11 -: 8]);

        b[1][69 -: 3] = 3'b101;
        b[1][0] = 1'bx;
        $display("b1=%h", b[1]);

        dflt[9][0] = 1'b1;
        $display("d9=%h d10=%h n=%0d", dflt[9], dflt[10], dflt.num());

        sa["k"] = '0;
        sa["k"][3] = 1'b1;
        name = "m";
        sa[name][69:66] = 4'h5;
        $display("sk=%h sm=%b n=%0d", sa["k"], sa[name][69:64], sa.num());
        $finish;
    end
endmodule
