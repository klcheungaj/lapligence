module tb;
    typedef enum bit [1:0] { ZERO=0, ONE=1 } code_t;
    typedef enum logic signed [2:0] { NEGATIVE=-1, NIL=0 } signed_t;
    code_t memory[0:2];
    signed_t signed_memory[0:1];
    bit [6:0] bits_memory[0:1];
    initial begin
        memory = '{ZERO, ONE, ONE};
        signed_memory = '{NIL, NIL};
        $readmemh("enum.hex", memory);
        if (memory[0] !== ONE || memory[1] !== ONE || memory[2] !== ONE)
            $fatal(1, "two-state normalization must not hide high enum overflow");
        $readmemh("signed.hex", signed_memory);
        if (signed_memory[0] !== NEGATIVE || signed_memory[1] !== NIL)
            $fatal(1, "signed enum redundant extension");
        $readmemh("two_state.hex", bits_memory);
        if (bits_memory[0] !== 7 || bits_memory[1] !== 1)
            $fatal(1, "two-state memory conversion");
        $readmemh("enum_unknown.hex", memory);
        if (memory[0] !== ZERO || memory[1] !== ONE || memory[2] !== ZERO)
            $fatal(1, "two-state enum X/Z words convert to zero before membership");
        $display("MEMORY_ENUM_CONVERSION_PASS");
        $finish(0);
    end
endmodule
