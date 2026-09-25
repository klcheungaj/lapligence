// Duplicate explicit indices remain illegal even when type keys cover all elements.
module tb;
    int values[2];
    int seed;
    initial values = '{0:seed, (1-1):seed, int:seed};
endmodule
