// llg-test-fixture: SYN-019 post-2009 builtin rejection
// $countbits is not part of the IEEE 1800-2009 system-function set.
module tb;
    initial begin
        $display("%0d", $countbits(4'b0011, 1'b1));
        $finish;
    end
endmodule
