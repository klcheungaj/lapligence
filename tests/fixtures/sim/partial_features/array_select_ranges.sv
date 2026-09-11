module tb;
    logic [15:8] down[0:1];
    logic [-4:3] up[0:1];
    int i;
    initial begin
        down[0]=8'ha5; up[0]=8'h96; i=10;
        $display("read %b %b %h %h",down[0][i],up[0][-3],down[0][15:12],up[0][-4:-1]);
        down[0][15:12]<=4'h3;
        down[0][8]<=0;
        up[0][-4:-1]<=4'ha;
        up[0][3]<=1;
        #1 $display("write %h %h",down[0],up[0]);
        $finish;
    end
endmodule
