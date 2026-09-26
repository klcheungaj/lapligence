// llg-test-fixture: tests/fixtures/sim/syn024_tagged_patterns/tagged_runtime.sv
// IEEE 1800-2009 7.3.2, 11.9, 12.6: a tagged pattern tests the tag before
// its payload; a successful binding has the selected member's type and scope.
module tb;
    typedef struct packed {
        logic [3:0] opcode;
        logic [3:0] operand;
    } instruction_t;
    typedef union tagged packed {
        void invalid;
        logic [7:0] valid;
        instruction_t instruction;
    } choice_t;
    typedef union tagged packed {
        void stop;
        choice_t nested;
    } outer_t;

    choice_t choice;
    outer_t outer;
    logic [7:0] result;
    int checks, calls;

    function automatic choice_t sample();
        calls++;
        return choice;
    endfunction

    initial begin
        checks = 0;
        calls = 0;
        choice = tagged valid 8'h5a;

        if (sample() matches .whole &&& whole.valid == 8'h5a) begin
            if (whole.valid !== 8'h5a) $fatal(1, "whole tagged binding type");
            checks++;
        end else $fatal(1, "whole tagged binding/filter");
        if (calls != 1) $fatal(1, "tagged source was sampled twice");

        if (choice matches .*) checks++;
        else $fatal(1, "whole tagged wildcard");

        if (choice matches tagged invalid) $fatal(1, "wrong void tag");
        if (choice matches tagged instruction '{opcode: 4'hx, operand: 4'hz})
            $fatal(1, "inactive structure data");

        choice = tagged invalid;
        if (choice matches tagged invalid) checks++;
        else $fatal(1, "void tag");
        if (choice matches tagged valid 8'hxx) $fatal(1, "inactive valid data");

        choice = tagged instruction '{opcode: 4'hc, operand: 4'ha};
        if (choice matches tagged instruction
            '{opcode: 4'hc, operand: .operand_value} &&&
            operand_value == 4'ha) begin
            result = {4'hc, operand_value};
            checks++;
        end else $fatal(1, "structure payload binding/filter");
        if (result !== 8'hca) $fatal(1, "structure payload type");

        outer = tagged nested (tagged instruction '{opcode: 4'hc, operand: 4'ha});
        if (outer matches tagged nested
            (tagged instruction '{opcode: 4'hc, operand: .nested_operand}) &&&
            nested_operand == 4'ha) checks++;
        else $fatal(1, "nested instruction payload");

        outer = tagged stop;
        if (outer matches tagged stop) checks++;
        else $fatal(1, "nested void arm");
        if (outer matches tagged nested .inactive_nested)
            $fatal(1, "inactive nested member was bound");

        result = choice matches tagged valid .false_payload &&&
            false_payload == 8'h5a ? false_payload : 8'h00;
        if (result !== 8'h00) $fatal(1, "false conditional arm");
        checks++;

        choice = tagged valid 8'h5a;
        result = choice matches tagged valid .true_payload &&&
            true_payload == 8'h5a ? true_payload : 8'h00;
        if (result !== 8'h5a) $fatal(1, "true conditional arm");
        checks++;

        if (choice matches tagged valid .payload &&& payload == 8'h00)
            $fatal(1, "failed filter entered true arm");
        else checks++;

        choice = tagged valid 8'hx5;
        if (choice matches tagged valid 8'hx5) checks++;
        else $fatal(1, "active X data exact match");

        if (checks != 10) $fatal(1, "check count %0d", checks);
        $display("tagged_runtime=pass checks=%0d calls=%0d", checks, calls);
        $finish(0);
    end
endmodule
