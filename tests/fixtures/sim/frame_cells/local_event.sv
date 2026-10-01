module tb;
    initial begin
        automatic integer local_value = 7;
        $display("waiting %0d", local_value);
        @(local_value);
        $display("bad");
    end
    initial begin
        #1;
        $display("finished");
        $finish(0);
    end
endmodule
