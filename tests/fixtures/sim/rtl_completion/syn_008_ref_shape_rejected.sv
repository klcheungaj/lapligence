// llg-test-fixture: IEEE 1800-2009 §§23.2.2.2, 23.3.3.
// A ref port requires matching fixed-array storage; this extent mismatch is
// the only invalidity in the connection.
typedef logic [7:0] lane_t;
typedef lane_t row_t [0:1];
typedef lane_t wrong_row_t [0:2];

module child(ref row_t value);
    initial value[0] = 8'h5a;
endmodule

module tb;
    wrong_row_t value;
    child u(.value(value));
endmodule
