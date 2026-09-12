module child(output string value);
    initial begin
        value = "alpha";
        #1 value = "alpha";
        #1 value = "beta";
        #1 value = "beta";
    end
endmodule

module tb;
    string result;
    child dut(.value(result));

    initial begin
        #1;
        #0 $display("%s", result);
        #1;
        #0 $display("%s", result);
        #1;
        #0 $display("%s", result);
        $finish(0);
    end
endmodule
