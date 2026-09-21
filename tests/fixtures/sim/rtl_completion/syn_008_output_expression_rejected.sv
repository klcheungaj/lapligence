// llg-test-fixture: IEEE 1364-2001 §§12.3.6, 12.3.9; IEEE 1800-2009 §23.3.3.
// An output actual must retain a legal assignable terminal shape; a
// conditional expression is a single invalid connection fault.
module child(output logic [7:0] result);
    assign result = 8'h5a;
endmodule

module tb;
    logic [7:0] left;
    logic [7:0] right;
    logic select;
    child u(.result(select ? left : right));
endmodule
