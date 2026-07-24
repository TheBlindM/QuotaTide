import nodemailer from "nodemailer";

export class Mailer {
  constructor(config) {
    this.config = config;
    this.enabled = Boolean(
      config.host && config.from && config.to.length && config.user && config.pass,
    );
    this.transporter = this.enabled
      ? nodemailer.createTransport({
          host: config.host,
          port: config.port,
          secure: config.secure,
          auth: { user: config.user, pass: config.pass },
        })
      : null;
  }

  async send(subject, text) {
    if (!this.enabled) {
      return { status: "not_configured", detail: "SMTP 未完整配置" };
    }
    try {
      await this.transporter.sendMail({
        from: this.config.from,
        to: this.config.to.join(", "),
        subject,
        text,
      });
      return { status: "sent", detail: "" };
    } catch (error) {
      return { status: "failed", detail: error.message };
    }
  }
}
