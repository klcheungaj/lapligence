module tb;
    reg a;
    reg b;

    initial begin
        a = 0;
        b = 0;
        $display("active %0d %0d", a, b);
        a = 1;
        b <= 1;
        $display("active %0d %0d", a, b);
        #0 $display("inactive %0d %0d", a, b);
        $strobe("postponed %0d %0d", a, b);
        #1 $finish;
    end
endmodule
