// llg-test-fixture: tests/fixtures/sim/memory_editions/enum_wide.sv
module tb;
    typedef enum logic [129:0] {
        E0 = 130'h1,
        E1 = 130'h2,
        E2 = 130'h3,
        E3 = 130'h4
    } e_t;
    e_t mem [0:3];

    initial begin
        $readmemh("enum_wide.mem", mem);
        if (mem[0] !== E0 || mem[1] !== E1 || mem[2] !== E2 || mem[3] !== E3)
            $display("FAIL wide enum memory");
        else
            $display("PASS wide enum memory");
    end
endmodule
