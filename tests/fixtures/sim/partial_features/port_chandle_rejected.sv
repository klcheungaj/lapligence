module child(input chandle value);
endmodule

module tb;
    chandle handle;
    child dut(.value(handle));
endmodule
