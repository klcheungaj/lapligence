module tb;
    int a,b;
    initial begin
        b=7; a<=#4 b; b=9;
        $display("issued %0t %0d %0d",$time,a,b);
        #2 $display("pending %0t %0d",$time,a);
        #3 $display("committed %0t %0d",$time,a);
        $finish;
    end
endmodule
