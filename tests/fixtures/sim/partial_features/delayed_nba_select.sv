module tb;
    logic [15:0] a=0;
    logic [7:0] memory[0:1];
    int i;
    initial begin
        memory[0]=0; memory[1]=0;
        i=0;
        a[3:0]<=#3 4'ha;
        a[7:4]<=#3 4'hb;
        a[i+8]<=#3 1'b1;
        memory[i]<=#3 8'h42;
        i=1;
        a[15:12]=4'hc;
        #3 a[3:0]<=4'hd;
        #1 $display("%0t %h %h %h",$time,a,memory[0],memory[1]);
        $finish(0);
    end
endmodule
