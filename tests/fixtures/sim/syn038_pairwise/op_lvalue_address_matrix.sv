// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/op_lvalue_address_matrix.sv
typedef struct packed { bit bank; bit lane; } key_t;
typedef struct packed { logic [7:0] value; logic [7:0] guard; } record_t;

module tb;
    localparam key_t KEY0 = key_t'(2'b00);
    localparam key_t KEY1 = key_t'(2'b01);
    localparam key_t KEY2 = key_t'(2'b10);

    bit choose;
    logic [7:0] conditional_slice [0:2];
    logic [7:0] conditional_concat [0:2];
    logic conditional_pattern [0:2];
    record_t equality_field [0:2];
    logic [7:0] equality_slice [0:2];
    logic [7:0] equality_concat [0:2];
    logic equality_pattern [0:2];
    record_t cast_field [0:2];
    logic [7:0] cast_slice [0:2];
    logic [7:0] cast_concat [0:2];
    logic cast_pattern [0:2];
    logic [7:0] pattern_element [0:2];
    logic [7:0] pattern_concat [0:2];
    logic pattern_lvalue [0:2];

    initial begin
        choose = 1'b0;

        conditional_slice[KEY0] = 8'h12;
        conditional_slice[KEY1] = 8'h34;
        conditional_slice[choose ? KEY1 : KEY0][4 +: 4] = 4'ha;
        if (conditional_slice[KEY0] !== 8'ha2 || conditional_slice[KEY1] !== 8'h34)
            $fatal(1, "conditional row slice low address");
        choose = 1'b1;
        conditional_slice[choose ? KEY1 : KEY0][4 +: 4] = 4'hb;
        if (conditional_slice[KEY0] !== 8'ha2 || conditional_slice[KEY1] !== 8'hb4)
            $fatal(1, "conditional row slice high address");

        choose = 1'b0;
        conditional_concat[KEY0] = 8'h12;
        conditional_concat[KEY1] = 8'h34;
        conditional_concat[KEY2] = 8'h56;
        {conditional_concat[choose ? KEY1 : KEY0], conditional_concat[KEY2]} = 16'ha5c3;
        if (conditional_concat[KEY0] !== 8'ha5 || conditional_concat[KEY1] !== 8'h34 ||
            conditional_concat[KEY2] !== 8'hc3)
            $fatal(1, "conditional concatenation low address");
        choose = 1'b1;
        {conditional_concat[choose ? KEY1 : KEY0], conditional_concat[KEY2]} = 16'hb6d4;
        if (conditional_concat[KEY0] !== 8'ha5 || conditional_concat[KEY1] !== 8'hb6 ||
            conditional_concat[KEY2] !== 8'hd4)
            $fatal(1, "conditional concatenation high address");

        choose = 1'b0;
        conditional_pattern[KEY0] = 1'b0;
        conditional_pattern[KEY1] = 1'b0;
        conditional_pattern[KEY2] = 1'b1;
        '{conditional_pattern[choose ? KEY1 : KEY0], conditional_pattern[KEY2]} = 2'b10;
        if (conditional_pattern[KEY0] !== 1'b1 || conditional_pattern[KEY1] !== 1'b0 ||
            conditional_pattern[KEY2] !== 1'b0)
            $fatal(1, "conditional positional pattern low address");
        choose = 1'b1;
        '{conditional_pattern[choose ? KEY1 : KEY0], conditional_pattern[KEY2]} = 2'b10;
        if (conditional_pattern[KEY0] !== 1'b1 || conditional_pattern[KEY1] !== 1'b1 ||
            conditional_pattern[KEY2] !== 1'b0)
            $fatal(1, "conditional positional pattern high address");

        choose = 1'b0;
        equality_field[2'b00] = '{value: 8'h12, guard: 8'h21};
        equality_field[2'b01] = '{value: 8'h34, guard: 8'h43};
        equality_field[choose == 1'b1].value = 8'ha5;
        if (equality_field[2'b00].value !== 8'ha5 || equality_field[2'b01].value !== 8'h34 ||
            equality_field[2'b00].guard !== 8'h21 || equality_field[2'b01].guard !== 8'h43)
            $fatal(1, "equality field low address");
        choose = 1'b1;
        equality_field[choose == 1'b1].value = 8'hb6;
        if (equality_field[2'b00].value !== 8'ha5 || equality_field[2'b01].value !== 8'hb6 ||
            equality_field[2'b00].guard !== 8'h21 || equality_field[2'b01].guard !== 8'h43)
            $fatal(1, "equality field high address");

        choose = 1'b0;
        equality_slice[2'b00] = 8'h12;
        equality_slice[2'b01] = 8'h34;
        equality_slice[choose inside {1'b1}][4 +: 4] = 4'ha;
        if (equality_slice[2'b00] !== 8'ha2 || equality_slice[2'b01] !== 8'h34)
            $fatal(1, "inside row slice low address");
        choose = 1'b1;
        equality_slice[choose inside {1'b1}][4 +: 4] = 4'hb;
        if (equality_slice[2'b00] !== 8'ha2 || equality_slice[2'b01] !== 8'hb4)
            $fatal(1, "inside row slice high address");

        choose = 1'b0;
        equality_concat[2'b00] = 8'h12;
        equality_concat[2'b01] = 8'h34;
        equality_concat[2'b10] = 8'h56;
        {equality_concat[choose == 1'b1], equality_concat[2'b10]} = 16'ha5c3;
        if (equality_concat[2'b00] !== 8'ha5 || equality_concat[2'b01] !== 8'h34 ||
            equality_concat[2'b10] !== 8'hc3)
            $fatal(1, "equality concatenation low address");
        choose = 1'b1;
        {equality_concat[choose == 1'b1], equality_concat[2'b10]} = 16'hb6d4;
        if (equality_concat[2'b00] !== 8'ha5 || equality_concat[2'b01] !== 8'hb6 ||
            equality_concat[2'b10] !== 8'hd4)
            $fatal(1, "equality concatenation high address");

        choose = 1'b0;
        equality_pattern[2'b00] = 1'b0;
        equality_pattern[2'b01] = 1'b0;
        equality_pattern[2'b10] = 1'b1;
        '{equality_pattern[choose inside {1'b1}], equality_pattern[2'b10]} = 2'b10;
        if (equality_pattern[2'b00] !== 1'b1 || equality_pattern[2'b01] !== 1'b0 ||
            equality_pattern[2'b10] !== 1'b0)
            $fatal(1, "inside positional pattern low address");
        choose = 1'b1;
        '{equality_pattern[choose inside {1'b1}], equality_pattern[2'b10]} = 2'b10;
        if (equality_pattern[2'b00] !== 1'b1 || equality_pattern[2'b01] !== 1'b1 ||
            equality_pattern[2'b10] !== 1'b0)
            $fatal(1, "inside positional pattern high address");

        choose = 1'b0;
        cast_field[KEY0] = '{value: 8'h12, guard: 8'h21};
        cast_field[KEY1] = '{value: 8'h34, guard: 8'h43};
        cast_field[key_t'(choose)].value = 8'ha5;
        if (cast_field[KEY0].value !== 8'ha5 || cast_field[KEY1].value !== 8'h34 ||
            cast_field[KEY0].guard !== 8'h21 || cast_field[KEY1].guard !== 8'h43)
            $fatal(1, "cast field low address");
        choose = 1'b1;
        cast_field[key_t'(choose)].value = 8'hb6;
        if (cast_field[KEY0].value !== 8'ha5 || cast_field[KEY1].value !== 8'hb6 ||
            cast_field[KEY0].guard !== 8'h21 || cast_field[KEY1].guard !== 8'h43)
            $fatal(1, "cast field high address");

        choose = 1'b0;
        cast_slice[KEY0] = 8'h12;
        cast_slice[KEY1] = 8'h34;
        cast_slice[key_t'(choose)][4 +: 4] = 4'ha;
        if (cast_slice[KEY0] !== 8'ha2 || cast_slice[KEY1] !== 8'h34)
            $fatal(1, "cast row slice low address");
        choose = 1'b1;
        cast_slice[key_t'(choose)][4 +: 4] = 4'hb;
        if (cast_slice[KEY0] !== 8'ha2 || cast_slice[KEY1] !== 8'hb4)
            $fatal(1, "cast row slice high address");

        choose = 1'b0;
        cast_concat[KEY0] = 8'h12;
        cast_concat[KEY1] = 8'h34;
        cast_concat[KEY2] = 8'h56;
        {cast_concat[key_t'(choose)], cast_concat[KEY2]} = 16'ha5c3;
        if (cast_concat[KEY0] !== 8'ha5 || cast_concat[KEY1] !== 8'h34 ||
            cast_concat[KEY2] !== 8'hc3)
            $fatal(1, "cast concatenation low address");
        choose = 1'b1;
        {cast_concat[key_t'(choose)], cast_concat[KEY2]} = 16'hb6d4;
        if (cast_concat[KEY0] !== 8'ha5 || cast_concat[KEY1] !== 8'hb6 ||
            cast_concat[KEY2] !== 8'hd4)
            $fatal(1, "cast concatenation high address");

        choose = 1'b0;
        cast_pattern[KEY0] = 1'b0;
        cast_pattern[KEY1] = 1'b0;
        cast_pattern[KEY2] = 1'b1;
        '{cast_pattern[key_t'(choose)], cast_pattern[KEY2]} = 2'b10;
        if (cast_pattern[KEY0] !== 1'b1 || cast_pattern[KEY1] !== 1'b0 ||
            cast_pattern[KEY2] !== 1'b0)
            $fatal(1, "cast positional pattern low address");
        choose = 1'b1;
        '{cast_pattern[key_t'(choose)], cast_pattern[KEY2]} = 2'b10;
        if (cast_pattern[KEY0] !== 1'b1 || cast_pattern[KEY1] !== 1'b1 ||
            cast_pattern[KEY2] !== 1'b0)
            $fatal(1, "cast positional pattern high address");

        choose = 1'b0;
        pattern_element[KEY0] = 8'h12;
        pattern_element[KEY1] = 8'h34;
        pattern_element[key_t'{1'b0, choose}] = 8'ha5;
        if (pattern_element[KEY0] !== 8'ha5 || pattern_element[KEY1] !== 8'h34)
            $fatal(1, "assignment pattern element low address");
        choose = 1'b1;
        pattern_element[key_t'{1'b0, choose}] = 8'hb6;
        if (pattern_element[KEY0] !== 8'ha5 || pattern_element[KEY1] !== 8'hb6)
            $fatal(1, "assignment pattern element high address");

        choose = 1'b0;
        pattern_concat[KEY0] = 8'h12;
        pattern_concat[KEY1] = 8'h34;
        pattern_concat[KEY2] = 8'h56;
        {pattern_concat[key_t'{1'b0, choose}], pattern_concat[KEY2]} = 16'ha5c3;
        if (pattern_concat[KEY0] !== 8'ha5 || pattern_concat[KEY1] !== 8'h34 ||
            pattern_concat[KEY2] !== 8'hc3)
            $fatal(1, "assignment pattern concatenation low address");
        choose = 1'b1;
        {pattern_concat[key_t'{1'b0, choose}], pattern_concat[KEY2]} = 16'hb6d4;
        if (pattern_concat[KEY0] !== 8'ha5 || pattern_concat[KEY1] !== 8'hb6 ||
            pattern_concat[KEY2] !== 8'hd4)
            $fatal(1, "assignment pattern concatenation high address");

        choose = 1'b0;
        pattern_lvalue[KEY0] = 1'b0;
        pattern_lvalue[KEY1] = 1'b0;
        pattern_lvalue[KEY2] = 1'b1;
        '{pattern_lvalue[key_t'{1'b0, choose}], pattern_lvalue[KEY2]} = 2'b10;
        if (pattern_lvalue[KEY0] !== 1'b1 || pattern_lvalue[KEY1] !== 1'b0 ||
            pattern_lvalue[KEY2] !== 1'b0)
            $fatal(1, "assignment pattern positional pattern low address");
        choose = 1'b1;
        '{pattern_lvalue[key_t'{1'b0, choose}], pattern_lvalue[KEY2]} = 2'b10;
        if (pattern_lvalue[KEY0] !== 1'b1 || pattern_lvalue[KEY1] !== 1'b1 ||
            pattern_lvalue[KEY2] !== 1'b0)
            $fatal(1, "assignment pattern positional pattern high address");

        $display("conditional=%h,%h,%b equality=%h,%h,%b cast=%h,%h,%b pattern=%b,%h,%b",
                 conditional_slice[KEY0], conditional_concat[KEY1], conditional_pattern[KEY2],
                 equality_field[2'b00].value, equality_slice[2'b01], equality_pattern[2'b01],
                 cast_field[KEY1].value, cast_slice[KEY1], cast_pattern[KEY1],
                 pattern_element[KEY1], pattern_concat[KEY2], pattern_lvalue[KEY1]);
        $finish(0);
    end
endmodule
