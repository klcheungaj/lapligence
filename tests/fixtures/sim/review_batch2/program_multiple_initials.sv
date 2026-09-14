// R05: only the last initial cancels this program's detached descendants.
program worker;
    initial begin
        fork
            begin #2; tb.before_end++; end
            begin #6; tb.after_end++; end
        join_none
        #1;
    end
    initial begin #4; tb.other_initial++; end
endprogram
program keeper;
    initial #8;
endprogram
module tb;
    int before_end = 0, after_end = 0, other_initial = 0;
    worker p();
    keeper k();
    final $display("before=%0d after=%0d initial=%0d",
                   before_end, after_end, other_initial);
endmodule
