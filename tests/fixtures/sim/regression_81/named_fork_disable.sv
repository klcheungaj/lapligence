module tb;
    initial begin
        fork : workers
            begin
                #5;
                $display("WRONG: cancelled worker ran");
            end
        join_none
        disable workers;
        $display("after disable");
        #10;
        $display("no late worker");
        $finish(0);
    end
endmodule
