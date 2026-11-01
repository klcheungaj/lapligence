// Decision A19-D68: the initial statement of a sequential UDP is an object.
// vpi_handle(vpiInitial) of a UDP definition with an initial statement
// returns a vpiInitial process whose vpiStmt is a blocking vpiAssignment
// (vpiOpType vpiAssignmentOp); its vpiRhs is a one-bit binary constant
// holding the initial value and its vpiLhs the definition's output
// declaration (a vpiIODecl). A definition without an initial statement,
// sequential or combinational, returns NULL without an error.
//
// IEEE 1800-2009 37.32 (SystemVerilog-1800-2009.txt L59798-59816) draws a
// relation from a UDP definition to an "initial", and 29.3.3
// (L48761-48763): "The sequential UDP initial statement specifies the value
// of the output port when simulation begins. ... The statement that follows
// shall be an assignment statement that assigns a single-bit literal value
// to the output port." 37.59 detail 1 (L61124): "vpiOpType shall return
// vpiAssignmentOp for normal assignments". The object and statement types
// follow industry-standard behaviour. Some simulators leave vpiOpType of
// this assignment undefined, read the literal as a 32-bit decimal constant
// and give the assignment a variable named after the instantiating scope as
// its left side; llg follows the clause text (a normal assignment of a
// single-bit binary literal) and gives the definition's output declaration
// (37.32) as the left side, since the output of a definition belongs to no
// instantiating scope.
//
// Build A19-D68_udp_initial_statement.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d68()` declared as a system task).
primitive ff (q, d, clk);
    output q; reg q;
    input d, clk;
    initial q = 1'b1;
    table
        ? r : ? : -;
        0 f : ? : 0;
        1 f : ? : 1;
    endtable
endprimitive

primitive latch (q, d, en);
    output q; reg q;
    input d, en;
    table
        ? 0 : ? : -;
        0 1 : ? : 0;
        1 1 : ? : 1;
    endtable
endprimitive

primitive inv (y, a);
    output y;
    input a;
    table
        0 : 1;
        1 : 0;
    endtable
endprimitive

module tb;
    logic clk = 0, d = 0, en = 0;
    wire q, l, y;
    ff u_ff (q, d, clk);
    latch u_latch (l, d, en);
    inv u_inv (y, d);
    initial begin
        #1 $d68;
        $finish;
    end
endmodule
