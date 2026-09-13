module tb;
    logic [15:0] a=0;
    initial begin
        a[3:0]<=4'ha; a[7:4]<=4'hb;
        a[11:8]<=#0 4'hc;
        a[15:12]=4'hd;
        #0 $display("inactive %h",a);
        $strobe("postponed %h",a);
        #1 $finish(0);
    end
endmodule
