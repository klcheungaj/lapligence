module tb;
    real delay_value, argument;
    initial begin
        argument=-1.0;
        delay_value=$sqrt(argument);
        #delay_value;
        $finish(0);
    end
endmodule
