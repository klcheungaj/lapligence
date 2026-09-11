module tb;
    logic [15:8] down;
    logic [-4:3] up;
    logic [128:0] wide_index;
    integer i;
    initial begin
        down=8'ha5; up=8'h96; i=10;
        $display("read %b %b %h %h",down[i],up[-3],down[15:12],up[-4:-1]);
        $display("indexed %h %h",down[10+:4],up[-3+:4]);
        $display("partial %b %b",down[17:14],up[-6:-3]);
        down[8]=0; up[3]=1;
        down[15:12]=4'h3; up[-4:-1]=4'ha;
        down[10+:3]<=3'b110; up[-3+:3]<=3'b101;
        i=100; down[i]<=1; up[i]<=1;
        i='x; down[i]<=1; up[i]<=1;
        wide_index=129'd1<<128; down[wide_index]<=1; up[wide_index]<=1;
        #1 $display("write %h %h",down,up);
        $display("reverse %b %b",down[12-:3],up[-1-:3]);
        $finish;
    end
endmodule
