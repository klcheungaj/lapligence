module tb;
    import "DPI-C" clash = function int first(input int value);
    import "DPI-C" clash = function real second(input real value);

    initial $finish;
endmodule
