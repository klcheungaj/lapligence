// llg-test-fixture: ADV-017 assertion control. control.c ($setup, $ctl,
// $report) applies every vpi_control assertion operation (IEEE 1800-2009
// 39.5.1, 39.5.2) to a concurrent, an immediate, a deferred and a
// sub-instance assertion, and reports the control, attempt and assertion
// system callbacks (39.4.1, 39.4.2) with the 40.5.3 counters. Expected
// output: control.out (derivation in readme.md).
`timescale 1ns/1ns
module sub(input logic clk, input logic x);
    s_x: assert property (@(posedge clk) x);
endmodule

module tb;
    logic clk = 0, a = 1, b = 0;
    int p_pass = 0, p_fail = 0, im_pass = 0, im_fail = 0, df_pass = 0, df_fail = 0;

    always #5 clk = ~clk;

    function void df_passed();
        df_pass++;
    endfunction
    function void df_failed();
        df_fail++;
    endfunction

    p: assert property (@(posedge clk) a |=> b) p_pass++;
        else p_fail++;
    always @(posedge clk) begin
        im: assert (b) im_pass++;
            else im_fail++;
        df: assert #0 (b) df_passed();
            else df_failed();
    end
    sub u_sub(clk, b);

    initial begin
        $setup;
        #8 $ctl(1);
        #20 $ctl(2);
        #10 $ctl(3);
        #10 $ctl(4);
        #2 b = 1'b1;
        #8 $ctl(5);
        #12 b = 1'b0;
        #8 $ctl(6);
        #10 $ctl(7);
        #2 b = 1'b1;
        #8 $ctl(8);
        #10 $ctl(9);
        #10 $ctl(10);
        #4 $asserton;
        #18 $display("p_pass=%0d p_fail=%0d im_pass=%0d im_fail=%0d df_pass=%0d df_fail=%0d",
                     p_pass, p_fail, im_pass, im_fail, df_pass, df_fail);
        $report;
        $finish;
    end
endmodule
