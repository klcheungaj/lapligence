// llg-test-fixture: ADV-020 simulator action callbacks. actions.c
// ($act_register, $act_pli, $act_signal, $act_remove) registers cbError,
// cbPLIError and cbSignal records, checks that the reasons llg does not raise
// are refused, and reports each callback with the error status
// vpi_chk_error() gives inside it (IEEE 1800-2009 38.36.3). Expected output:
// actions.out.
module tb;
    int x = 0;
    initial begin
        $act_register;
        $act_pli;
        $display("after act_pli");
        #1 $error("first error %0d", x);
        $warning("a warning");
        $info("an info");
        x = 1;
        assert (x == 2);
        assert (x == 2) else $display("else action");
        #1 $act_signal;
        #1 $act_remove;
        $error("unobserved error");
        $display("done");
        $finish;
    end
endmodule
