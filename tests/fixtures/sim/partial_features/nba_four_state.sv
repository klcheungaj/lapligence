module tb;
    logic [64:0] a='0;
    logic [7:0] memory[0:1];
    int base;
    initial begin
        memory[0]=0; memory[1]=0;
        base=60;
        a[base+:5]<=#2 5'b1xz01;
        a[3:0]<=#2 4'bz10x;
        a[100]<=#2 1'b1;
        a[32'bx]<=#2 1'b1;
        memory[0][3:0]<=#2 4'ha;
        memory[0][7:4]<=#2 4'hb;
        memory[2]<=#2 8'hff;
        base=0;
        #3 $display("%b %b %h %h",a[64:60],a[3:0],memory[0],memory[1]);
        $finish;
    end
endmodule
