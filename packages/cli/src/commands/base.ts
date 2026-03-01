import { Command } from 'clipanion';
import { isApiError } from '../api/error.js';
import { ColorOption, configureColor } from '../console.js';
import {
  configureLogger,
  getLogger,
  type Logger,
  LogLevelOption,
  LogVerboseOption,
} from '../log.js';

export abstract class BaseCommand extends Command {
  abstract readonly name: string;

  readonly color = ColorOption;
  readonly logLevel = LogLevelOption;
  readonly logVerbose = LogVerboseOption;

  private _logger: Logger | null = null;
  protected get logger(): Logger {
    if (this._logger == null) {
      throw new Error('Should configure logger before use');
    }
    return this._logger;
  }

  abstract run(): Promise<number | boolean | void>;

  async execute() {
    configureColor(this.color);
    await configureLogger({
      level: this.logLevel,
      verbose: this.logVerbose,
    });
    this._logger = getLogger(this.name);
    try {
      const ret = await this.run();
      if (typeof ret === 'number') {
        return ret;
      }
      if (typeof ret === 'boolean') {
        return ret ? 0 : 1;
      }
      return 0;
    } catch (error) {
      // Ignore logging for operation error, because it's intent to be already logged in operation.
      if (!isApiError(error)) {
        this._logger.error(`"${this.name}" command failed with error: {error}`, { error });
      }
      return 1;
    }
  }
}
