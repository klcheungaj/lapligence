// R05: pending Re-NBA work is not drained after all program initials end.
program worker;
    initial begin
        #1;
        tb.pending <= 1;
    end
endprogram
module tb;
    int pending = 0;
    worker p();
    final $display("pending=%0d", pending);
endmodule
