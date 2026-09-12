module child(input string value);
    initial begin
        #1 $display("%s", value);
    end
endmodule

module tb;
    string source = "hello";
    child dut(.value(source));

    initial begin
        #2 $finish(0);
    end
endmodule
