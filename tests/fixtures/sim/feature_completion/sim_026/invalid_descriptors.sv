// SIM-026 A02/A03: every input function on a closed descriptor, descriptor
// 0, an unknown descriptor, multichannel descriptors, an unopened descriptor
// and a write-only file. Each returns its failure code (EOF for $fgetc,
// $ungetc and $fscanf; 0 for $fgets and $fread; -1 for $ftell, $fseek,
// $rewind and $feof) and leaves its destination unchanged (IEEE 1800-2009
// 21.3.1, 21.3.4-21.3.7).
module tb;
  integer fd, c, mcd;
  logic [7:0] a;
  logic [7:0] m[0:1];
  string s, msg;
  initial begin
    fd = $fopen("closed.txt", "w");
    $fwrite(fd, "1 2\n");
    $fclose(fd);
    fd = $fopen("closed.txt", "r");
    $fclose(fd);
    a = 8'd5;
    s = "keep";
    m[0] = 8'h77;
    c = $fgetc(fd);
    $display("A getc=%0d", c);
    c = $ungetc(65, fd);
    $display("B ungetc=%0d", c);
    c = $fgets(s, fd);
    $display("C fgets=%0d s=%s", c, s);
    c = $fscanf(fd, "%d", a);
    $display("D fscanf=%0d a=%0d", c, a);
    c = $fread(a, fd);
    $display("E fread=%0d a=%0d", c, a);
    c = $fread(m, fd);
    $display("F fread=%0d m0=%h", c, m[0]);
    $display("G tell=%0d seek=%0d rewind=%0d eof=%0d", $ftell(fd), $fseek(fd, 0, 0), $rewind(fd),
             $feof(fd));
    c = $ferror(fd, msg);
    $display("H ferror=%0d message=%0d", c != 0, msg.len() != 0);
    $display("I zero=%0d x=%0d", $fgetc(0), $fscanf(32'hx, "%d", a));
    mcd = 2;
    $display("J mcd=%0d stdout=%0d unopened=%0d", $fgetc(mcd), $fscanf(1, "%d", a),
             $fgetc(int'(32'h8000_0055)));
    fd = $fopen("writeonly.txt", "w");
    $fwrite(fd, "1 2\n");
    c = $fscanf(fd, "%d", a);
    $display("K getc=%0d fscanf=%0d a=%0d", $fgetc(fd), c, a);
    $fclose(fd);
    $finish;
  end
endmodule
