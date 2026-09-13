module tb;
    int a;
    initial begin a<=#3 11; end
    initial begin
        #3 $display("active %0d",a);
        $strobe("postponed %0d",a);
        #1 $display("later %0d",a);
        $finish(0);
    end
endmodule
