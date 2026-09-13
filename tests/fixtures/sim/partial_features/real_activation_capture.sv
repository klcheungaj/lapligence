// IEEE 1800-2009 6.21 and 9.3: a retained fork activation owns a copied
// automatic real value after the declaring procedural block continues.
module tb;
    initial begin
        automatic real first;
        automatic real second;
        first = 1.5;
        second = 2.5;
        fork
            begin
                #1 $display("real first %0.1f", first);
            end
            begin
                #1 $display("real second %0.1f", second);
            end
        join_none
        first = 9.5;
        second = 8.5;
        #2;
        wait fork;
        if (first != 9.5 || second != 8.5)
            $display("FAIL real_activation_capture %0.1f %0.1f", first, second);
        else
            $display("PASS real_activation_capture");
        $finish(0);
    end
endmodule
