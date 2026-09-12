// llg-test-fixture: tests/fixtures/sim/file_io/file_input.sv
`timescale 1ns/1ps
module tb;
  integer fd;
  integer binary_fd;
  integer status;
  integer gets_status;
  string line;
  string word;
  reg [7:0] character;
  reg [63:0] packed_line;
  reg signed [31:0] decimal;
  reg [15:0] hexadecimal;
  reg [23:0] wide;
  reg [7:0] descending [3:0];
  reg [7:0] ascending [0:3];
  reg [7:0] scanned [0:1];
  integer index0;
  integer index1;

  initial begin
    fd = $fopen("file_input.txt", "rb");
    character = $fgetc(fd);
    status = $ungetc(character, fd);
    gets_status = $fgets(line, fd);
    if (status !== 65 || gets_status !== 7 || line.len() !== 7 ||
        line.getc(0) !== 8'h41 || line.getc(1) !== 8'h20 ||
        line.getc(2) !== 8'h6c || line.getc(3) !== 8'h69 ||
        line.getc(4) !== 8'h6e || line.getc(5) !== 8'h65 ||
        line.getc(6) !== 8'h0a) begin
      $display("FAIL line status=%0d gets=%0d line=%s", status, gets_status, line);
      $finish;
    end

    status = $fseek(fd, 0, 0);
    gets_status = $fgets(packed_line, fd);
    if (status !== 0 || gets_status !== 7 || packed_line !== 64'h0041206c696e650a) begin
      $display("FAIL packed line status=%0d gets=%0d value=%h", status,
               gets_status, packed_line);
      $finish;
    end
    $display("packed_line=%h bytes=%0d", packed_line, gets_status);
    status = $fseek(fd, 7, 0);

    status = $fscanf(fd, "%d %4h %*s %s", decimal, hexadecimal, word);
    if (status !== 3 || decimal !== 123 || hexadecimal !== 16'h001x ||
        word.len() !== 4 || word.getc(0) !== 8'h77 || word.getc(1) !== 8'h6f ||
        word.getc(2) !== 8'h72 || word.getc(3) !== 8'h64) begin
      $display("FAIL scan status=%0d decimal=%0d hexadecimal=%h word=%s",
               status, decimal, hexadecimal, word);
      $finish;
    end
    $display("scan=%0d decimal=%0d hexadecimal=%h word=%s",
             status, decimal, hexadecimal, word);

    status = $sscanf("42 xz", "%d %h", decimal, hexadecimal);
    if (status !== 2 || decimal !== 42 || hexadecimal !== 16'h00xz) begin
      $display("FAIL sscanf status=%0d decimal=%0d hexadecimal=%h",
               status, decimal, hexadecimal);
      $finish;
    end
    $display("sscanf=%0d decimal=%0d hexadecimal=%h", status, decimal, hexadecimal);

    index0 = 0;
    index1 = 1;
    status = $sscanf("aa bb", "%h %h", scanned[index0], scanned[index1]);
    if (status !== 2 || scanned[0] !== 8'haa || scanned[1] !== 8'hbb) begin
      $display("FAIL selected scan status=%0d values=%h,%h", status,
               scanned[0], scanned[1]);
      $finish;
    end
    $display("selected=%h,%h bytes=%0d", scanned[0], scanned[1], status);

    binary_fd = $fopen("file_input.bin", "rb");
    status = $fread(wide, binary_fd);
    if (status !== 3 || wide !== 24'h123456) begin
      $display("FAIL wide status=%0d wide=%h", status, wide);
      $finish;
    end
    $display("wide=%h bytes=%0d", wide, status);

    descending[3] = 8'hxx;
    descending[2] = 8'hxx;
    descending[1] = 8'hxx;
    descending[0] = 8'hxx;
    status = $fseek(binary_fd, 0, 0);
    status = $fread(descending, binary_fd, 2, 2);
    if (status !== 2 || descending[3] !== 8'hxx || descending[2] !== 8'h12 ||
        descending[1] !== 8'h34 || descending[0] !== 8'hxx) begin
      $display("FAIL descending status=%0d values=%h,%h,%h,%h", status,
               descending[3], descending[2], descending[1], descending[0]);
      $finish;
    end
    $display("descending=%h,%h,%h,%h bytes=%0d", descending[3], descending[2],
             descending[1], descending[0], status);

    ascending[0] = 8'hxx;
    ascending[1] = 8'hxx;
    ascending[2] = 8'hxx;
    ascending[3] = 8'hxx;
    status = $fseek(binary_fd, 0, 0);
    status = $fread(ascending, binary_fd);
    if (status !== 4 || ascending[0] !== 8'h12 || ascending[1] !== 8'h34 ||
        ascending[2] !== 8'h56 || ascending[3] !== 8'h78) begin
      $display("FAIL ascending status=%0d values=%h,%h,%h,%h", status,
               ascending[0], ascending[1], ascending[2], ascending[3]);
      $finish;
    end
    $display("ascending=%h,%h,%h,%h bytes=%0d", ascending[0], ascending[1],
             ascending[2], ascending[3], status);

    $fclose(fd);
    $fclose(binary_fd);
    $finish;
  end
endmodule
